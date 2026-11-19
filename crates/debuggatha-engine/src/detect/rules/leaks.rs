//! `leak-mock-restore`: a test file that overrides state nothing in it, or in the test runner's
//! configuration (`modules::TestConfig`), puts back, so state leaks into the next test.
//!
//! - **Spies** (`vi.spyOn`, `jest.spyOn`, `sinon.spy|stub|mock`) are put back by `mockRestore`,
//!   `restore`, `mockClear` or `mockReset` on the spy, by a global restore/clear/reset call, by any
//!   such call in a hook, by `using`, or by `restoreMocks`, `clearMocks`, `resetMocks` or
//!   `mockReset` set in the configuration. Clearing counts: the rule says "restore or clear".
//! - **Stubbed globals and environment** (`vi.stubGlobal`, `vi.stubEnv`) by the matching unstub.
//! - **`process.env` assigned inside a function** by an assignment or `delete` of the same key in
//!   an `after` hook, or by a second write in the same function. One at the top of the file is the
//!   file's setup and is left alone.
//!
//! Every hit is medium: whether the override matters depends on test order and isolation, and a
//! configuration the review does not hold (a diff review) cannot be read.

use std::collections::BTreeSet;

use oxc_allocator::Allocator;
use oxc_ast::AstKind;
use oxc_ast::ast::{
    ArrowFunctionExpression, AssignmentExpression, AssignmentOperator, AssignmentTarget,
    CallExpression, Expression, Function, UnaryExpression, UnaryOperator, VariableDeclaration,
    VariableDeclarationKind, VariableDeclarator,
};
use oxc_ast_visit::{Visit, walk};
use oxc_parser::Parser;
use oxc_semantic::{ScopeFlags, Semantic, SemanticBuilder};
use oxc_span::GetSpan;

use super::calls::one_per_line;
use super::js::{Lines, make_hit, symbol_index};
use crate::detect::hit::Hit;
use crate::detect::input::DetectorInput;
use crate::detect::runner::Registry;
use crate::scan_scope::is_test_path;
use crate::syntax::Language;
use crate::syntax::oxc::source_type;
use debuggatha_core::review_engine::Confidence;

/// The rules this detector answers for.
pub const RULES: &[&str] = &["leak-mock-restore"];

/// Registers the detector.
pub fn register(registry: &mut Registry) {
    registry.register_group(RULES, detect);
}

const AFTER_HOOKS: &[&str] = &["afterEach", "afterAll", "after", "teardown"];
const BEFORE_HOOKS: &[&str] = &["beforeEach", "beforeAll", "before", "setup"];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    Spy,
    Global,
    EnvStub,
    EnvSet,
}

/// Where a spy was kept: a variable, or a property (`this.spy`, `ctx.spy`).
#[derive(Clone, PartialEq, Eq, Debug)]
enum Bound {
    Symbol(usize),
    Prop(String),
}

struct Override {
    span: (usize, usize),
    kind: Kind,
    bound: Option<Bound>,
    using: bool,
    /// For a stubbed global or environment variable: its name (`"?"` when it is not a literal).
    name: Option<String>,
    /// For a spy: the variable the spied object starts from, for the fresh-per-test check.
    target: Option<usize>,
    /// For `process.env`: the key (`"*"` for the whole object or a computed key) and the function
    /// the write is in.
    env: Option<(String, u32)>,
}

/// A call that puts things back.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Seen {
    /// `restoreAllMocks`, `clearAllMocks`, `resetAllMocks`, `sinon.restore()`.
    RestoreAll,
    /// A restore or clear of any receiver inside a hook.
    InHook,
    UnstubGlobals,
    UnstubEnvs,
}

/// A function being visited: where it is, and whether it is the suite's own setup.
struct Frame {
    start: u32,
    end: u32,
    suite: bool,
}

struct Scan<'a> {
    semantic: &'a Semantic<'a>,
    /// Start offset of a creation's expression to where it is kept.
    pending: Vec<(u32, Bound, bool)>,
    overrides: Vec<Override>,
    restored: Vec<Bound>,
    /// What the file calls to put things back, anywhere in it.
    seen: BTreeSet<Seen>,
    /// `process.env` writes in `after` hooks: keys (`"*"` is the whole object).
    after_env: BTreeSet<String>,
    /// Every write or `delete` of an env key: (function, key, start).
    env_writes: Vec<(u32, String, usize)>,
    hooks: usize,
    after: usize,
    functions: Vec<Frame>,
    /// Start offsets of callbacks of `describe` and `beforeAll`: the suite's setup, not a test.
    suite_fns: BTreeSet<u32>,
    declaring_using: bool,
    before_each: usize,
    /// Variables assigned a `new` object inside a `beforeEach`, and variables given a value in any
    /// other way: the first and not the second hold a fresh object in every test.
    fresh: BTreeSet<usize>,
    stale: BTreeSet<usize>,
    /// Names stubbed again inside an `after` hook (`g:` global, `e:` environment): the reset.
    after_stubs: BTreeSet<String>,
}

/// `describe(..)`, `describe.each(..)(..)`, `describe.skip(..)`, `beforeAll(..)`: calls whose
/// callback is the suite's setup.
fn is_suite_call(callee: &Expression<'_>) -> bool {
    let mut e = callee;
    loop {
        match e {
            Expression::CallExpression(c) => e = &c.callee,
            Expression::StaticMemberExpression(m) => {
                if matches!(m.property.name.as_str(), "beforeAll" | "hoisted") {
                    return true;
                }
                e = &m.object;
            }
            Expression::Identifier(id) => {
                return matches!(
                    id.name.as_str(),
                    "describe" | "suite" | "context" | "beforeAll" | "before"
                );
            }
            _ => return false,
        }
    }
}

fn hook_name<'e>(callee: &'e Expression<'_>) -> Option<&'e str> {
    match callee {
        Expression::Identifier(id) => Some(id.name.as_str()),
        Expression::StaticMemberExpression(m) => Some(m.property.name.as_str()),
        _ => None,
    }
}

fn is_process_env(e: &Expression<'_>) -> bool {
    matches!(e, Expression::StaticMemberExpression(m)
        if m.property.name == "env"
            && matches!(&m.object, Expression::Identifier(id) if id.name == "process"))
}

/// The key of `process.env.K`; `"?"` for `process.env[k]` (a key that is not a name) and `"*"` for
/// `process.env` itself; `None` when the target is not the environment.
fn env_key(target: &AssignmentTarget<'_>) -> Option<String> {
    match target {
        AssignmentTarget::StaticMemberExpression(m) => {
            if is_process_env(&m.object) {
                Some(m.property.name.to_string())
            } else if m.property.name == "env"
                && matches!(&m.object, Expression::Identifier(id) if id.name == "process")
            {
                Some("*".to_owned())
            } else {
                None
            }
        }
        AssignmentTarget::ComputedMemberExpression(m) => {
            is_process_env(&m.object).then(|| "?".into())
        }
        _ => None,
    }
}

impl Scan<'_> {
    fn bound_of(&self, e: &Expression<'_>) -> Option<Bound> {
        match e {
            Expression::Identifier(id) => symbol_index(self.semantic, id).map(Bound::Symbol),
            Expression::StaticMemberExpression(m) => Some(Bound::Prop(m.property.name.to_string())),
            _ => None,
        }
    }

    fn take_pending(&mut self, start: u32) -> (Option<Bound>, bool) {
        self.pending
            .iter()
            .position(|(s, _, _)| *s == start)
            .map_or((None, false), |i| {
                let (_, bound, using) = self.pending.swap_remove(i);
                (Some(bound), using)
            })
    }

    fn creation(&mut self, call: &CallExpression<'_>, kind: Kind) {
        let (bound, using) = self.take_pending(call.span.start);
        let first = call.arguments.first().and_then(|a| a.as_expression());
        let name = matches!(kind, Kind::Global | Kind::EnvStub).then(|| match first {
            Some(Expression::StringLiteral(l)) => l.value.display().to_string(),
            _ => "?".to_owned(),
        });
        if self.after > 0
            && let Some(name) = &name
        {
            // Stubbed again after the test: that is the reset, not a new override.
            let prefix = if kind == Kind::Global { "g" } else { "e" };
            self.after_stubs.insert(format!("{prefix}:{name}"));
            return;
        }
        if self.in_suite_scope() {
            return;
        }
        let mut target = None;
        if kind == Kind::Spy
            && let Some(first) = first
        {
            if self.is_test_local(first) {
                return;
            }
            target = self.root_symbol(first);
        }
        self.overrides.push(Override {
            span: (call.span.start as usize, call.span.end as usize),
            kind,
            bound,
            using,
            name,
            target,
            env: None,
        });
    }

    /// The variable an expression like `a.b.c` starts from.
    fn root_symbol(&self, target: &Expression<'_>) -> Option<usize> {
        let mut e = target;
        loop {
            match e {
                Expression::StaticMemberExpression(m) => e = &m.object,
                Expression::ComputedMemberExpression(m) => e = &m.object,
                Expression::ParenthesizedExpression(p) => e = &p.expression,
                _ => break,
            }
        }
        match e {
            Expression::Identifier(id) => symbol_index(self.semantic, id),
            _ => None,
        }
    }

    fn enter(&mut self, start: u32, end: u32) {
        let suite = self.suite_fns.contains(&start);
        self.functions.push(Frame { start, end, suite });
    }

    /// At the top of the file, or only inside `describe` / `beforeAll` callbacks: what is set there
    /// is the file's setup, shared by design, not state one test leaves for the next.
    fn in_suite_scope(&self) -> bool {
        self.functions.iter().all(|f| f.suite)
    }

    /// Is the object a spy is put on made by the test itself (declared inside it, and not an alias
    /// of something outside)? Spying on it cannot reach another test.
    fn is_test_local(&self, target: &Expression<'_>) -> bool {
        let Some(test) = self.functions.iter().find(|f| !f.suite) else {
            return false;
        };
        let mut e = target;
        loop {
            match e {
                Expression::StaticMemberExpression(m) => e = &m.object,
                Expression::ComputedMemberExpression(m) => e = &m.object,
                Expression::ParenthesizedExpression(p) => e = &p.expression,
                _ => break,
            }
        }
        let Expression::Identifier(id) = e else {
            return false;
        };
        let Some(symbol) = self
            .semantic
            .scoping()
            .get_reference(id.reference_id())
            .symbol_id()
        else {
            return false;
        };
        let scoping = self.semantic.scoping();
        let declared = scoping.symbol_span(symbol).start;
        if !(test.start..test.end).contains(&declared) {
            return false;
        }
        match self
            .semantic
            .nodes()
            .get_node(scoping.symbol_declaration(symbol))
            .kind()
        {
            // `const c = console` is the global under another name.
            AstKind::VariableDeclarator(d) => !d.init.as_ref().is_some_and(|i| {
                matches!(
                    i,
                    Expression::Identifier(_)
                        | Expression::StaticMemberExpression(_)
                        | Expression::ComputedMemberExpression(_)
                )
            }),
            _ => true,
        }
    }

    fn function_start(&self) -> u32 {
        self.functions.last().map_or(0, |f| f.start)
    }
}

impl<'a> Visit<'a> for Scan<'a> {
    fn visit_function(&mut self, it: &Function<'a>, flags: ScopeFlags) {
        self.enter(it.span.start, it.span.end);
        walk::walk_function(self, it, flags);
        self.functions.pop();
    }

    fn visit_arrow_function_expression(&mut self, it: &ArrowFunctionExpression<'a>) {
        self.enter(it.span.start, it.span.end);
        walk::walk_arrow_function_expression(self, it);
        self.functions.pop();
    }

    fn visit_variable_declaration(&mut self, it: &VariableDeclaration<'a>) {
        let before = self.declaring_using;
        self.declaring_using = matches!(
            it.kind,
            VariableDeclarationKind::Using | VariableDeclarationKind::AwaitUsing
        );
        walk::walk_variable_declaration(self, it);
        self.declaring_using = before;
    }

    fn visit_variable_declarator(&mut self, it: &VariableDeclarator<'a>) {
        if let (oxc_ast::ast::BindingPattern::BindingIdentifier(id), Some(init)) =
            (&it.id, it.init.as_ref())
            && let Some(symbol) = id.symbol_id.get()
        {
            self.stale.insert(symbol.index());
            self.pending.push((
                init.span().start,
                Bound::Symbol(symbol.index()),
                self.declaring_using,
            ));
        }
        walk::walk_variable_declarator(self, it);
    }

    fn visit_assignment_expression(&mut self, it: &AssignmentExpression<'a>) {
        if it.operator == AssignmentOperator::Assign {
            let kept = match &it.left {
                AssignmentTarget::AssignmentTargetIdentifier(id) => {
                    if let Some(symbol) = symbol_index(self.semantic, id) {
                        if self.before_each > 0 && matches!(it.right, Expression::NewExpression(_))
                        {
                            self.fresh.insert(symbol);
                        } else {
                            self.stale.insert(symbol);
                        }
                    }
                    symbol_index(self.semantic, id).map(Bound::Symbol)
                }
                AssignmentTarget::StaticMemberExpression(m) => {
                    Some(Bound::Prop(m.property.name.to_string()))
                }
                _ => None,
            };
            if let Some(bound) = kept {
                self.pending.push((it.right.span().start, bound, false));
            }
            if let Some(key) = env_key(&it.left) {
                self.env_write(&key, it.span.start, it.span.end, true);
            }
        }
        walk::walk_assignment_expression(self, it);
    }

    fn visit_unary_expression(&mut self, it: &UnaryExpression<'a>) {
        if it.operator == UnaryOperator::Delete {
            match &it.argument {
                Expression::StaticMemberExpression(m) if is_process_env(&m.object) => {
                    self.env_write(m.property.name.as_str(), it.span.start, it.span.end, false);
                }
                Expression::ComputedMemberExpression(m) if is_process_env(&m.object) => {
                    self.env_write("?", it.span.start, it.span.end, false);
                }
                _ => {}
            }
        }
        walk::walk_unary_expression(self, it);
    }

    fn visit_call_expression(&mut self, it: &CallExpression<'a>) {
        let hook =
            hook_name(&it.callee).filter(|n| AFTER_HOOKS.contains(n) || BEFORE_HOOKS.contains(n));
        let after = hook.is_some_and(|n| AFTER_HOOKS.contains(&n));
        if is_suite_call(&it.callee) {
            for argument in &it.arguments {
                match argument.as_expression() {
                    Some(Expression::FunctionExpression(f)) => {
                        self.suite_fns.insert(f.span.start);
                    }
                    Some(Expression::ArrowFunctionExpression(a)) => {
                        self.suite_fns.insert(a.span.start);
                    }
                    _ => {}
                }
            }
        }
        if let Expression::StaticMemberExpression(m) = &it.callee {
            let method = m.property.name.as_str();
            let object = match &m.object {
                Expression::Identifier(id) => Some(id.name.as_str()),
                _ => None,
            };
            match (object, method) {
                (Some("vi" | "jest"), "spyOn") | (Some("sinon"), "spy" | "stub" | "mock") => {
                    self.creation(it, Kind::Spy);
                }
                (Some("vi"), "stubGlobal") => self.creation(it, Kind::Global),
                (Some("vi"), "stubEnv") => self.creation(it, Kind::EnvStub),
                (Some("vi" | "jest"), "restoreAllMocks" | "clearAllMocks" | "resetAllMocks")
                | (Some("sinon"), "restore") => {
                    self.seen.insert(Seen::RestoreAll);
                }
                (Some("vi"), "unstubAllGlobals") => {
                    self.seen.insert(Seen::UnstubGlobals);
                }
                (Some("vi"), "unstubAllEnvs") => {
                    self.seen.insert(Seen::UnstubEnvs);
                }
                _ => {}
            }
            if matches!(
                method,
                "mockRestore" | "restore" | "mockClear" | "mockReset"
            ) && it.arguments.is_empty()
            {
                if let Some(bound) = self.bound_of(&m.object) {
                    self.restored.push(bound);
                }
                if self.hooks > 0 {
                    self.seen.insert(Seen::InHook);
                }
            }
        }
        if hook.is_some() {
            let each = usize::from(hook == Some("beforeEach"));
            self.hooks += 1;
            self.after += usize::from(after);
            self.before_each += each;
            walk::walk_call_expression(self, it);
            self.hooks -= 1;
            self.after -= usize::from(after);
            self.before_each -= each;
        } else {
            walk::walk_call_expression(self, it);
        }
    }
}

impl Scan<'_> {
    fn env_write(&mut self, key: &str, start: u32, end: u32, assigns: bool) {
        if self.after > 0 {
            self.after_env.insert(key.to_owned());
        }
        if self.in_suite_scope() {
            return;
        }
        let function = self.function_start();
        self.env_writes
            .push((function, key.to_owned(), start as usize));
        if assigns {
            self.overrides.push(Override {
                span: (start as usize, end as usize),
                kind: Kind::EnvSet,
                bound: None,
                using: false,
                name: None,
                target: None,
                env: Some((key.to_owned(), function)),
            });
        }
    }

    fn stubbed_after(&self, prefix: char, ov: &Override) -> bool {
        let name = ov.name.as_deref().unwrap_or("?");
        self.after_stubs.contains(&format!("{prefix}:{name}"))
            || self.after_stubs.contains(&format!("{prefix}:?"))
    }

    fn env_put_back(&self, ov: &Override) -> bool {
        let Some((key, function)) = &ov.env else {
            return false;
        };
        let wildcard = self.after_env.contains("*");
        let in_hook =
            wildcard || self.after_env.contains(key) || (key == "*" && !self.after_env.is_empty());
        // Written again in the same function, which is set-and-put-back.
        let again = self.env_writes.iter().any(|(f, k, at)| {
            f == function && (k == key || k == "*" || key == "*") && *at != ov.span.0
        });
        in_hook || again
    }
}

/// Runs the detector over one file.
#[must_use]
pub fn detect(input: &DetectorInput<'_>) -> Vec<(&'static str, Hit)> {
    let Some(syntax) = input.syntax else {
        return Vec::new();
    };
    if !is_test_path(input.file)
        || !matches!(
            syntax.language,
            Language::JavaScript | Language::Jsx | Language::TypeScript | Language::Tsx
        )
    {
        return Vec::new();
    }
    let allocator = Allocator::default();
    let parsed = Parser::new(
        &allocator,
        input.content,
        source_type(syntax.language, false),
    )
    .parse();
    if parsed.fatal_error {
        return Vec::new();
    }
    let semantic = SemanticBuilder::new()
        .with_build_nodes(true)
        .build(&parsed.program)
        .semantic;
    let mut scan = Scan {
        semantic: &semantic,
        pending: Vec::new(),
        overrides: Vec::new(),
        restored: Vec::new(),
        seen: BTreeSet::new(),
        after_env: BTreeSet::new(),
        env_writes: Vec::new(),
        hooks: 0,
        after: 0,
        functions: Vec::new(),
        suite_fns: BTreeSet::new(),
        declaring_using: false,
        before_each: 0,
        fresh: BTreeSet::new(),
        stale: BTreeSet::new(),
        after_stubs: BTreeSet::new(),
    };
    scan.visit_program(&parsed.program);
    let config = input
        .modules
        .map(|index| index.test_config_for(input.file))
        .unwrap_or_default();
    let lines = Lines::new(input.content);
    let mut hits = Vec::new();
    for ov in &scan.overrides {
        let put_back = match ov.kind {
            Kind::Spy => {
                config.restore_mocks
                    || scan.seen.contains(&Seen::RestoreAll)
                    || scan.seen.contains(&Seen::InHook)
                    || ov.using
                    || ov.bound.as_ref().is_some_and(|b| scan.restored.contains(b))
                    || ov
                        .target
                        .is_some_and(|t| scan.fresh.contains(&t) && !scan.stale.contains(&t))
            }
            Kind::Global => {
                config.unstub_globals
                    || scan.seen.contains(&Seen::UnstubGlobals)
                    || scan.stubbed_after('g', ov)
            }
            Kind::EnvStub => {
                config.unstub_envs
                    || scan.seen.contains(&Seen::UnstubEnvs)
                    || scan.stubbed_after('e', ov)
            }
            Kind::EnvSet => scan.env_put_back(ov),
        };
        if !put_back
            && let Some(hit) = make_hit(
                input,
                &lines,
                ov.span.0,
                ov.span.1,
                Confidence::Medium,
                None,
            )
        {
            hits.push((RULES[0], hit, ov.span.0));
        }
    }
    one_per_line(hits)
}
