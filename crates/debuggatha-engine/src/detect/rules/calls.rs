//! Group B: rules about calls and tests of the JavaScript/TypeScript family, written over oxc's
//! tree. A call is a node, so a callee, its arguments and the body of a test callback are exact.
//!
//! Each rule keeps the contract the TypeScript engine gives (extensions, confidence, the span of
//! the hit, the excerpt of the whole line, one hit per line and rule); differences are listed in
//! `docs/PARITY.md`.

use std::collections::BTreeSet;

use oxc_allocator::Allocator;
use oxc_ast::ast::{
    Argument, ArrowFunctionBody, BinaryOperator, BindingIdentifier, CallExpression, Expression,
    FunctionBody, IdentifierName, IdentifierReference, ObjectProperty, PropertyKey,
    StaticMemberExpression, TryStatement,
};
use oxc_ast_visit::{Visit, walk};
use oxc_parser::Parser;
use oxc_span::GetSpan;

use super::js::{Lines, blank_comments, is_js_like, is_stringish, make_hit, range};
use crate::detect::hit::Hit;
use crate::detect::input::DetectorInput;
use crate::detect::runner::Registry;
use crate::scan_scope::is_test_path;
use crate::syntax::oxc::source_type;
use crate::syntax::{Language, Span};
use debuggatha_core::review_engine::Confidence;

/// The rules this group answers for.
pub const RULES: &[&str] = &[
    "no-async-foreach",
    "no-shell-interpolation",
    "no-open-cors",
    "no-error-leak",
    "async-route-errors",
    "no-focused-tests",
    "no-empty-tests",
    "no-assertionless-tests",
];

/// Registers the group.
pub fn register(registry: &mut Registry) {
    registry.register_group(RULES, detect);
}

const TEST_ROOTS: &[&str] = &["it", "test", "describe", "context", "suite"];
const ROUTE_METHODS: &[&str] = &["get", "post", "put", "patch", "delete", "all"];
/// What counts as an assertion in a test body: names, read from the code.
const ASSERTION: &[&str] = &[
    "expect", "assert", "should", "verify", "fail", "rejects", "tothrow", "snapshot",
];

/// The name an expression ends in: `res` or `this.res`.
fn tail_name<'e>(e: &'e Expression<'_>) -> Option<&'e str> {
    match e {
        Expression::Identifier(id) => Some(id.name.as_str()),
        Expression::StaticMemberExpression(m) => Some(m.property.name.as_str()),
        _ => None,
    }
}

/// The span start of that name (not of `this.`), as the old `\b` pattern began there.
fn tail_start(e: &Expression<'_>) -> usize {
    match e {
        Expression::StaticMemberExpression(m) => m.property.span.start as usize,
        _ => e.span().start as usize,
    }
}

fn is_async_function(e: &Expression<'_>) -> bool {
    match e {
        Expression::ArrowFunctionExpression(a) => a.r#async,
        Expression::FunctionExpression(f) => f.r#async,
        _ => false,
    }
}

/// The block body of a callback; an expression-bodied arrow has no block to inspect.
fn block_body<'e, 'a>(e: &'e Expression<'a>) -> Option<&'e FunctionBody<'a>> {
    match e {
        Expression::ArrowFunctionExpression(a) => match &a.body {
            ArrowFunctionBody::FunctionBody(b) => Some(b),
            _ => None,
        },
        Expression::FunctionExpression(f) => f.body.as_deref(),
        _ => None,
    }
}

/// `try` or `.catch(` anywhere below.
#[derive(Default)]
struct Catches(bool);
impl<'a> Visit<'a> for Catches {
    fn visit_try_statement(&mut self, _: &TryStatement<'a>) {
        self.0 = true;
    }
    fn visit_call_expression(&mut self, it: &CallExpression<'a>) {
        if let Expression::StaticMemberExpression(m) = &it.callee
            && m.property.name == "catch"
        {
            self.0 = true;
        }
        walk::walk_call_expression(self, it);
    }
}

/// Whether any name below looks like an assertion.
#[derive(Default)]
struct Asserts(bool);
impl Asserts {
    fn name(&mut self, name: &str) {
        let lower = name.to_ascii_lowercase();
        // `\bdone\b`: the callback's completion signal, `$done` included.
        if lower.trim_matches('$') == "done" || ASSERTION.iter().any(|w| lower.contains(w)) {
            self.0 = true;
        }
    }
}
impl<'a> Visit<'a> for Asserts {
    fn visit_identifier_reference(&mut self, it: &IdentifierReference<'a>) {
        self.name(it.name.as_str());
    }
    fn visit_identifier_name(&mut self, it: &IdentifierName<'a>) {
        self.name(it.name.as_str());
    }
    fn visit_binding_identifier(&mut self, it: &BindingIdentifier<'a>) {
        self.name(it.name.as_str());
    }
}

/// The first `err.stack` / `err.message` read below, by position.
#[derive(Default)]
struct Leak(Option<(usize, bool)>);
impl<'a> Visit<'a> for Leak {
    fn visit_static_member_expression(&mut self, it: &StaticMemberExpression<'a>) {
        let prop = it.property.name.as_str();
        if matches!(prop, "stack" | "message")
            && tail_name(&it.object).is_some_and(|n| matches!(n, "err" | "error" | "e" | "ex"))
        {
            let at = it.span.start as usize;
            if self.0.is_none_or(|(first, _)| at < first) {
                self.0 = Some((at, prop == "stack"));
            }
        }
        walk::walk_static_member_expression(self, it);
    }
}

/// Leaves of a `+` chain.
fn leaves<'e, 'a>(e: &'e Expression<'a>, out: &mut Vec<&'e Expression<'a>>) {
    if let Expression::BinaryExpression(b) = e
        && b.operator == BinaryOperator::Addition
    {
        leaves(&b.left, out);
        leaves(&b.right, out);
    } else {
        out.push(e);
    }
}

fn is_constant(e: &Expression<'_>) -> bool {
    match e {
        Expression::StringLiteral(_) | Expression::NumericLiteral(_) => true,
        Expression::TemplateLiteral(t) => t.expressions.is_empty(),
        _ => false,
    }
}

/// A command built from values: a template with `${}` or a `+` chain of text and something else.
fn builds_a_command(e: &Expression<'_>) -> bool {
    match e {
        Expression::TemplateLiteral(t) => !t.expressions.is_empty(),
        Expression::BinaryExpression(b) if b.operator == BinaryOperator::Addition => {
            let mut parts = Vec::new();
            leaves(e, &mut parts);
            parts.iter().any(|p| is_stringish(p)) && parts.iter().any(|p| !is_constant(p))
        }
        _ => false,
    }
}

/// `it(...)`, `test.only(...)`, `it.concurrent(...)`: the call of one test.
fn is_test_call(callee: &Expression<'_>) -> bool {
    match callee {
        Expression::Identifier(id) => matches!(id.name.as_str(), "it" | "test"),
        Expression::StaticMemberExpression(m) => {
            matches!(m.property.name.as_str(), "only" | "concurrent" | "fails")
                && matches!(&m.object, Expression::Identifier(id)
                    if matches!(id.name.as_str(), "it" | "test"))
        }
        _ => false,
    }
}

fn test_body<'c, 'a>(it: &'c CallExpression<'a>) -> Option<&'c FunctionBody<'a>> {
    if !is_test_call(&it.callee) {
        return None;
    }
    let mut args = it.arguments.iter().filter_map(Argument::as_expression);
    // The name of the test is text; a callback may follow anywhere after it.
    if !is_stringish(args.next()?) {
        return None;
    }
    args.find(|a| {
        matches!(
            a,
            Expression::ArrowFunctionExpression(_) | Expression::FunctionExpression(_)
        )
    })
    .and_then(block_body)
}

struct Collector<'a, 's> {
    code: String,
    lines: Lines<'s>,
    file: &'s str,
    input: &'a DetectorInput<'s>,
    tests: bool,
    hits: Vec<(&'static str, Hit, usize)>,
}

impl Collector<'_, '_> {
    fn push(&mut self, rule: &'static str, start: usize, end: usize, confidence: Confidence) {
        if let Some(hit) = make_hit(self.input, &self.lines, start, end, confidence, None) {
            self.hits.push((rule, hit, start));
        }
    }

    /// The offset just past the `(` that follows `from`, if only spaces lie between.
    fn after_paren(&self, from: usize) -> Option<usize> {
        let rest = self.code.get(from..)?;
        let trimmed = rest.trim_start();
        trimmed
            .starts_with('(')
            .then(|| from + (rest.len() - trimmed.len()) + 1)
    }

    /// The `.` (or the one of `?.`) just before a property name.
    fn dot_before(&self, property: usize) -> Option<usize> {
        let before = self.code.get(..property)?.trim_end();
        before.ends_with('.').then(|| before.len() - 1)
    }

    fn shell(&mut self, it: &CallExpression<'_>) {
        let (name, start) = match &it.callee {
            Expression::Identifier(id) => (id.name.as_str(), id.span.start as usize),
            Expression::StaticMemberExpression(m) => match &m.object {
                Expression::Identifier(o)
                    if matches!(o.name.as_str(), "child_process" | "childProcess" | "cp") =>
                {
                    (m.property.name.as_str(), o.span.start as usize)
                }
                _ => return,
            },
            _ => return,
        };
        if !matches!(name, "exec" | "execSync") {
            return;
        }
        if it
            .arguments
            .first()
            .and_then(Argument::as_expression)
            .is_some_and(builds_a_command)
            && let Some(open) = self.after_paren(it.callee.span().end as usize)
        {
            self.push("no-shell-interpolation", start, open, Confidence::Medium);
        }
    }

    fn error_leak(&mut self, it: &CallExpression<'_>) {
        let Expression::StaticMemberExpression(method) = &it.callee else {
            return;
        };
        if !matches!(method.property.name.as_str(), "json" | "send") {
            return;
        }
        // `res.json(...)` or `res.status(...).json(...)`.
        let response = match &method.object {
            Expression::CallExpression(c) => match &c.callee {
                Expression::StaticMemberExpression(s) if s.property.name == "status" => {
                    Some(&s.object)
                }
                _ => None,
            },
            other => Some(other),
        };
        let Some(response) = response else { return };
        if !tail_name(response).is_some_and(|n| matches!(n, "res" | "response")) {
            return;
        }
        let mut leak = Leak::default();
        for arg in &it.arguments {
            leak.visit_argument(arg);
        }
        if let Some((_, stack)) = leak.0
            && let Some(open) = self.after_paren(method.property.span.end as usize)
        {
            let confidence = if stack {
                Confidence::High
            } else {
                Confidence::Medium
            };
            self.push("no-error-leak", tail_start(response), open, confidence);
        }
    }

    fn route(&mut self, it: &CallExpression<'_>) {
        let Expression::StaticMemberExpression(m) = &it.callee else {
            return;
        };
        if !ROUTE_METHODS.contains(&m.property.name.as_str()) {
            return;
        }
        let Some(owner) = tail_name(&m.object) else {
            return;
        };
        let router = owner == "app"
            || owner == "router"
            || ["Router", "App"]
                .iter()
                .any(|s| owner.len() > s.len() && owner.ends_with(s));
        if !router {
            return;
        }
        let Some(at) = it
            .arguments
            .iter()
            .position(|a| a.as_expression().is_some_and(is_async_function))
        else {
            return;
        };
        let mut catches = Catches::default();
        for arg in &it.arguments[at..] {
            catches.visit_argument(arg);
        }
        if !catches.0 {
            let start = it.arguments[at].span().start as usize;
            self.push("async-route-errors", start, start + 5, Confidence::Medium);
        }
    }

    fn focused(&mut self, it: &CallExpression<'_>) {
        // `fit(`, `it.only(`, `describe.only.each(rows)(`, `test.describe.only(`.
        let mut names = Vec::new();
        let mut at = &it.callee;
        while let Expression::StaticMemberExpression(m) = at {
            names.push(m.property.name.as_str());
            at = &m.object;
        }
        let Expression::Identifier(root) = at else {
            return;
        };
        let root_name = root.name.as_str();
        let focused = if names.is_empty() {
            matches!(root_name, "fit" | "fdescribe")
        } else {
            TEST_ROOTS.contains(&root_name) && names.contains(&"only")
        };
        if focused && let Some(open) = self.after_paren(it.callee.span().end as usize) {
            self.push(
                "no-focused-tests",
                root.span.start as usize,
                open,
                Confidence::High,
            );
        }
    }

    fn test(&mut self, it: &CallExpression<'_>) {
        if !self.tests || !is_js_like(self.file) {
            return;
        }
        let Some(body) = test_body(it) else {
            return;
        };
        let start = it.callee.span().start as usize;
        let Some(open) = self.after_paren(it.callee.span().end as usize) else {
            return;
        };
        if body.statements.is_empty() && body.directives.is_empty() {
            self.push("no-empty-tests", start, open, Confidence::High);
            return;
        }
        let mut asserts = Asserts::default();
        asserts.visit_function_body(body);
        if !asserts.0 {
            // Low confidence: a helper such as `checkInvariants()` may assert on the test's behalf.
            self.push("no-assertionless-tests", start, open, Confidence::Low);
        }
    }
}

impl<'a> Visit<'a> for Collector<'a, '_> {
    fn visit_call_expression(&mut self, it: &CallExpression<'a>) {
        if is_js_like(self.file) {
            if let Expression::StaticMemberExpression(m) = &it.callee
                && m.property.name == "forEach"
                && it
                    .arguments
                    .first()
                    .and_then(Argument::as_expression)
                    .is_some_and(is_async_function)
                && let Some(dot) = self.dot_before(m.property.span.start as usize)
                && let Some(open) = self.after_paren(m.property.span.end as usize)
            {
                self.push("no-async-foreach", dot, open, Confidence::High);
            }
            if !self.tests {
                self.shell(it);
                self.error_leak(it);
                self.route(it);
                if let Expression::Identifier(id) = &it.callee
                    && id.name == "cors"
                    && it.arguments.is_empty()
                    && let Some(open) = self.after_paren(id.span.end as usize)
                {
                    self.push(
                        "no-open-cors",
                        id.span.start as usize,
                        open,
                        Confidence::High,
                    );
                }
            }
            self.focused(it);
            self.test(it);
        }
        walk::walk_call_expression(self, it);
    }

    fn visit_object_property(&mut self, it: &ObjectProperty<'a>) {
        // `origin: '*'` lets any site read the responses.
        if is_js_like(self.file)
            && !self.tests
            && !it.computed
            && let PropertyKey::StaticIdentifier(key) = &it.key
            && key.name == "origin"
            && match &it.value {
                Expression::StringLiteral(s) => s.value == "*",
                Expression::TemplateLiteral(t) => {
                    t.expressions.is_empty()
                        && t.quasis
                            .first()
                            .is_some_and(|q| q.value.cooked.as_ref().is_some_and(|c| c == "*"))
                }
                _ => false,
            }
        {
            let (start, _) = range(&**key);
            let (_, end) = range(&it.value);
            self.push("no-open-cors", start, end, Confidence::High);
        }
        walk::walk_object_property(self, it);
    }
}

/// Runs the group over one file.
#[must_use]
pub fn detect(input: &DetectorInput<'_>) -> Vec<(&'static str, Hit)> {
    let Some(syntax) = input.syntax else {
        return Vec::new();
    };
    let language = syntax.language;
    if !matches!(
        language,
        Language::JavaScript | Language::Jsx | Language::TypeScript | Language::Tsx
    ) {
        return Vec::new();
    }
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, input.content, source_type(language, false)).parse();
    if parsed.fatal_error {
        return Vec::new();
    }
    let comments: Vec<Span> = syntax
        .facts
        .as_ref()
        .map(|f| f.comments.clone())
        .unwrap_or_default();
    let mut collector = Collector {
        code: blank_comments(input.content, &comments),
        lines: Lines::new(input.content),
        file: input.file,
        input,
        tests: is_test_path(input.file),
        hits: Vec::new(),
    };
    collector.visit_program(&parsed.program);

    one_per_line(collector.hits)
}

/// Source order, one hit per line and rule (the rule of the TypeScript detectors, kept so that a
/// finding's line, and with it its ledger identity, does not move). Each hit comes with the byte
/// offset it starts at.
pub(in crate::detect) fn one_per_line(
    mut hits: Vec<(&'static str, Hit, usize)>,
) -> Vec<(&'static str, Hit)> {
    hits.sort_by_key(|(_, h, start)| (h.line, *start, h.columns.end));
    let mut seen = BTreeSet::new();
    hits.into_iter()
        .filter(|(rule, h, _)| seen.insert((*rule, h.line)))
        .map(|(rule, h, _)| (rule, h))
        .collect()
}
