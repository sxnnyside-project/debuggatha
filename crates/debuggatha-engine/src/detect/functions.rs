//! `require-await` and `no-loop-func`, over the tree and oxc's scope analysis. Each hit is what the
//! tree proves, no more.
//!
//! - `require-await`: an `async` function with no `await`, `for await` or `await using` of its own;
//!   medium when it hands back a call or a `new`, high otherwise. Left alone: a function whose type
//!   comes from where it is written (an argument, a property, a returned callback, a typed
//!   variable), one declaring `: Promise<T>`, generators, stubs, methods of a class with a parent
//!   and test code. Without types those cannot be told from an interface that asks for a Promise.
//! - `no-loop-func`: a function created in a loop that reads a function-scoped variable the loop
//!   shares between iterations (a `var` in the loop, or one written from the loop's start on). A
//!   `let` or `const` is never reported.
//!
//! What is left alone, and why, is stated next to each check and in `docs/DETECTOR_QUALITY.md`.

use std::collections::{BTreeMap, BTreeSet};

use oxc_allocator::Allocator;
use oxc_ast::ast::{
    ArrowFunctionExpression, CallExpression, Class, DoWhileStatement, Expression, ForInStatement,
    ForOfStatement, ForStatement, Function, MethodDefinition, PropertyDefinition, ReturnStatement,
    Statement, VariableDeclarationKind, WhileStatement,
};
use oxc_ast_visit::{Visit, walk};
use oxc_parser::Parser;
use oxc_semantic::{ScopeFlags, Semantic, SemanticBuilder, SymbolFlags};
use oxc_span::GetSpan;

use super::calls::one_per_line;
use super::hit::Hit;
use super::input::DetectorInput;
use super::js::{Lines, make_hit};
use super::runner::Registry;
use crate::scan_scope::is_test_path;
use crate::syntax::Language;
use crate::syntax::oxc::source_type;
use debuggatha_core::review_engine::Confidence;

/// The rules this group answers for.
pub const RULES: &[&str] = &["require-await", "no-loop-func"];

/// Registers the group.
pub fn register(registry: &mut Registry) {
    registry.register_group(RULES, detect);
}

/// Array methods that call their callback before they return: a function passed to one of them
/// in a loop cannot outlive the iteration.
pub const SYNCHRONOUS_CALLBACKS: &[&str] = &[
    "forEach",
    "map",
    "filter",
    "reduce",
    "reduceRight",
    "some",
    "every",
    "find",
    "findIndex",
    "findLast",
    "findLastIndex",
    "flatMap",
    "sort",
    "toSorted",
];

/// What the visitor learns about the `async` function it is inside of.
struct Frame {
    start: usize,
    head_end: usize,
    awaits: bool,
    /// A `return f(..)` or `return new X(..)`, or an arrow whose body is one.
    hands_back_a_call: bool,
    /// Does not take part in the rule: a generator, a method of a class with a parent, a body that
    /// is empty or only throws.
    /// Also set for a function that is not `async`: the rule is about `async` ones.
    exempt: bool,
}

struct Visitor<'a, 's> {
    semantic: &'a Semantic<'a>,
    input: &'a DetectorInput<'a>,
    lines: Lines<'s>,
    test_file: bool,
    frames: Vec<Frame>,
    /// Innermost loop of the current function (start, end), reset at a function boundary.
    loops: Vec<(usize, usize)>,
    /// Start offsets of functions that a class with a parent (or `override`) makes exempt.
    inherited: BTreeSet<u32>,
    /// Start offsets of functions whose type comes from where they are written: an argument, a
    /// property of an object, a returned callback, a typed variable. `async` there may be what an
    /// interface asks for, and without types that cannot be told apart, so `require-await` leaves
    /// them alone.
    contextual: BTreeSet<u32>,
    /// Start offsets of functions that cannot outlive the iteration they are written in.
    immediate: BTreeSet<u32>,
    /// Where a method's report starts: its `async` and name, not the `(` its function begins at.
    heads: BTreeMap<u32, u32>,
    /// Write references of each symbol, as start offsets (built once).
    writes: Vec<Vec<usize>>,
    hits: Vec<(&'static str, Hit, usize)>,
}

const fn range_of(span: oxc_span::Span) -> (usize, usize) {
    (span.start as usize, span.end as usize)
}

/// The function a class member holds, if its start is recorded as exempt.
fn function_start(e: &Expression<'_>) -> Option<u32> {
    match e {
        Expression::FunctionExpression(f) => Some(f.span.start),
        Expression::ArrowFunctionExpression(a) => Some(a.span.start),
        _ => None,
    }
}

/// The start of a function that is the whole body of an arrow (`() => async () => 1`).
fn expression_start(arrow: &ArrowFunctionExpression<'_>) -> Option<u32> {
    arrow.get_expression().and_then(function_start)
}

/// `: Promise<T>` or `: PromiseLike<T>` written on the function: the author declared the contract,
/// and `async` is what the usual TypeScript lint asks for there.
pub(super) fn declares_a_promise(annotation: Option<&oxc_ast::ast::TSTypeAnnotation<'_>>) -> bool {
    annotation.is_some_and(|a| {
        matches!(&a.type_annotation, oxc_ast::ast::TSType::TSTypeReference(r)
            if matches!(&r.type_name, oxc_ast::ast::TSTypeName::IdentifierReference(id)
                if matches!(id.name.as_str(), "Promise" | "PromiseLike")))
    })
}

pub(super) fn peel_parens<'e, 'a>(mut e: &'e Expression<'a>) -> &'e Expression<'a> {
    while let Expression::ParenthesizedExpression(p) = e {
        e = &p.expression;
    }
    e
}

fn is_call_like(e: &Expression<'_>) -> bool {
    matches!(
        peel_parens(e),
        Expression::CallExpression(_) | Expression::NewExpression(_)
    )
}

/// A body that does nothing (`{}`) or only throws is a stub: the `async` is there for the
/// signature it stands in for.
fn is_stub(statements: &[Statement<'_>]) -> bool {
    statements.is_empty() || matches!(statements, [Statement::ThrowStatement(_)])
}

impl Visitor<'_, '_> {
    fn push(&mut self, rule: &'static str, (start, end): (usize, usize), confidence: Confidence) {
        if let Some(hit) = make_hit(self.input, &self.lines, start, end, confidence, None) {
            self.hits.push((rule, hit, start));
        }
    }

    fn enter(&mut self, frame: Frame) {
        self.frames.push(frame);
    }

    /// Leaving a function: report an `async` one that never awaited.
    fn leave(&mut self) {
        let Some(frame) = self.frames.pop() else {
            return;
        };
        if !frame.awaits && !frame.exempt && !self.test_file {
            let confidence = if frame.hands_back_a_call {
                Confidence::Medium
            } else {
                Confidence::High
            };
            self.push("require-await", (frame.start, frame.head_end), confidence);
        }
    }

    /// Does the function read a variable the enclosing loop shares between its iterations?
    fn captures_shared_variable(&self, function: (usize, usize), lp: (usize, usize)) -> bool {
        let scoping = self.semantic.scoping();
        scoping.symbol_ids().any(|symbol| {
            let flags = scoping.symbol_flags(symbol);
            let (decl_start, _) = range_of(scoping.symbol_span(symbol));
            if (function.0..function.1).contains(&decl_start) {
                return false;
            }
            let read_inside = scoping
                .get_resolved_reference_ids(symbol)
                .iter()
                .map(|r| scoping.get_reference(*r))
                .any(|r| {
                    !r.is_type() && {
                        let at = range_of(self.semantic.nodes().get_node(r.node_id()).span()).0;
                        (function.0..function.1).contains(&at)
                    }
                });
            if !read_inside {
                return false;
            }
            // The rule's own wording lets a function "exclusively reference block-scoped
            // variables": a `let` or `const` may be written by the loop on purpose (a counter
            // the callbacks share), and every one of the sixteen cases on the corpus was that.
            if flags.contains(SymbolFlags::BlockScopedVariable) {
                return false;
            }
            let declared_in_loop = (lp.0..lp.1).contains(&decl_start);
            // A `var` of the loop is one binding for all of its iterations.
            if declared_in_loop {
                return flags.contains(SymbolFlags::FunctionScopedVariable);
            }
            self.writes[symbol.index()].iter().any(|&at| at >= lp.0)
        })
    }

    fn head_of(&self, function_start: u32) -> usize {
        self.heads
            .get(&function_start)
            .copied()
            .unwrap_or(function_start) as usize
    }

    fn check_loop_function(&mut self, start: u32, end: u32, head: usize, head_end: usize) {
        let Some(&lp) = self.loops.last() else {
            return;
        };
        if self.immediate.contains(&start) {
            return;
        }
        if self.captures_shared_variable((start as usize, end as usize), lp) {
            self.push("no-loop-func", (head, head_end), Confidence::High);
        }
    }

    /// Run `body` for a function boundary: loops of the outside do not reach in.
    fn boundary(&mut self, body: impl FnOnce(&mut Self)) {
        let outer = std::mem::take(&mut self.loops);
        body(self);
        self.loops = outer;
    }
}

impl<'a> Visit<'a> for Visitor<'a, '_> {
    fn visit_class(&mut self, it: &Class<'a>) {
        let parent = it.heritage.is_some() || !it.implements.is_empty();
        let before = self.inherited.clone();
        if parent {
            for element in &it.body.body {
                match element {
                    oxc_ast::ast::ClassElement::MethodDefinition(m) => {
                        self.inherited.insert(m.value.span.start);
                    }
                    oxc_ast::ast::ClassElement::PropertyDefinition(p) => {
                        if let Some(start) = p.value.as_ref().and_then(function_start) {
                            self.inherited.insert(start);
                        }
                    }
                    _ => {}
                }
            }
        }
        walk::walk_class(self, it);
        self.inherited = before;
    }

    fn visit_new_expression(&mut self, it: &oxc_ast::ast::NewExpression<'a>) {
        for argument in &it.arguments {
            if let Some(start) = argument.as_expression().and_then(function_start) {
                self.contextual.insert(start);
            }
        }
        walk::walk_new_expression(self, it);
    }

    fn visit_variable_declarator(&mut self, it: &oxc_ast::ast::VariableDeclarator<'a>) {
        if it.type_annotation.is_some()
            && let Some(start) = it.init.as_ref().and_then(function_start)
        {
            self.contextual.insert(start);
        }
        walk::walk_variable_declarator(self, it);
    }

    fn visit_jsx_expression_container(&mut self, it: &oxc_ast::ast::JSXExpressionContainer<'a>) {
        if let Some(start) = it.expression.as_expression().and_then(function_start) {
            self.contextual.insert(start);
        }
        walk::walk_jsx_expression_container(self, it);
    }

    fn visit_object_property(&mut self, it: &oxc_ast::ast::ObjectProperty<'a>) {
        if let Some(start) = function_start(&it.value) {
            self.contextual.insert(start);
            if it.method {
                self.heads.insert(start, it.span.start);
            }
        }
        walk::walk_object_property(self, it);
    }

    fn visit_method_definition(&mut self, it: &MethodDefinition<'a>) {
        // A decorator is part of the member's span, but the report belongs on the line of the
        // method itself (and an inline `debuggatha-ignore-next-line` sits above that line).
        let head = it.decorators.last().map_or(it.span.start, |d| {
            let after = &self.input.content[d.span.end as usize..];
            d.span.end + u32::try_from(after.len() - after.trim_start().len()).unwrap_or(0)
        });
        self.heads.insert(it.value.span.start, head);
        if it.r#override {
            self.inherited.insert(it.value.span.start);
        }
        walk::walk_method_definition(self, it);
    }

    fn visit_property_definition(&mut self, it: &PropertyDefinition<'a>) {
        if it.r#override
            && let Some(start) = it.value.as_ref().and_then(function_start)
        {
            self.inherited.insert(start);
        }
        walk::walk_property_definition(self, it);
    }

    fn visit_call_expression(&mut self, it: &CallExpression<'a>) {
        // `(function () {..})()`, `(() => ..)()`: runs now.
        if let Some(start) = function_start(peel_parens(&it.callee)) {
            self.immediate.insert(start);
        }
        // `.forEach(function () {..})` and its kin: runs before the call returns.
        if let Expression::StaticMemberExpression(m) = &it.callee
            && SYNCHRONOUS_CALLBACKS.contains(&m.property.name.as_str())
        {
            for argument in &it.arguments {
                if let Some(start) = argument.as_expression().and_then(function_start) {
                    self.immediate.insert(start);
                }
            }
        }
        for argument in &it.arguments {
            if let Some(start) = argument.as_expression().and_then(function_start) {
                self.contextual.insert(start);
            }
        }
        walk::walk_call_expression(self, it);
    }

    fn visit_function(&mut self, it: &Function<'a>, flags: ScopeFlags) {
        let Some(body) = it.body.as_ref() else {
            // A signature with no body (an overload, `declare`): nothing to read.
            walk::walk_function(self, it, flags);
            return;
        };
        let head_end = body.span.start as usize;
        let start = self.head_of(it.span.start);
        self.check_loop_function(it.span.start, it.span.end, start, head_end);
        self.enter(Frame {
            start,
            head_end,
            awaits: false,
            hands_back_a_call: false,
            exempt: !it.r#async
                || it.generator
                || self.inherited.contains(&it.span.start)
                || self.contextual.contains(&it.span.start)
                || declares_a_promise(it.return_type.as_deref())
                || is_stub(&body.statements),
        });
        self.boundary(|v| walk::walk_function(v, it, flags));
        self.leave();
    }

    fn visit_arrow_function_expression(&mut self, it: &ArrowFunctionExpression<'a>) {
        let head_end = it.body.span().start as usize;
        let start = self.head_of(it.span.start);
        self.check_loop_function(it.span.start, it.span.end, start, head_end);
        let expression = it.get_expression();
        if let Some(inner) = expression_start(it) {
            self.contextual.insert(inner);
        }
        self.enter(Frame {
            start,
            head_end,
            awaits: false,
            hands_back_a_call: expression.is_some_and(is_call_like),
            exempt: !it.r#async
                || self.inherited.contains(&it.span.start)
                || self.contextual.contains(&it.span.start)
                || declares_a_promise(it.return_type.as_deref())
                || it
                    .get_function_body()
                    .is_some_and(|b| is_stub(&b.statements)),
        });
        self.boundary(|v| walk::walk_arrow_function_expression(v, it));
        self.leave();
    }

    fn visit_await_expression(&mut self, it: &oxc_ast::ast::AwaitExpression<'a>) {
        if let Some(frame) = self.frames.last_mut() {
            frame.awaits = true;
        }
        walk::walk_await_expression(self, it);
    }

    fn visit_variable_declaration(&mut self, it: &oxc_ast::ast::VariableDeclaration<'a>) {
        if it.kind == VariableDeclarationKind::AwaitUsing
            && let Some(frame) = self.frames.last_mut()
        {
            frame.awaits = true;
        }
        walk::walk_variable_declaration(self, it);
    }

    fn visit_return_statement(&mut self, it: &ReturnStatement<'a>) {
        if let Some(start) = it.argument.as_ref().and_then(function_start) {
            self.contextual.insert(start);
        }
        if it.argument.as_ref().is_some_and(is_call_like)
            && let Some(frame) = self.frames.last_mut()
        {
            frame.hands_back_a_call = true;
        }
        walk::walk_return_statement(self, it);
    }

    fn visit_for_of_statement(&mut self, it: &ForOfStatement<'a>) {
        if it.r#await
            && let Some(frame) = self.frames.last_mut()
        {
            frame.awaits = true;
        }
        self.loops.push(range_of(it.span));
        walk::walk_for_of_statement(self, it);
        self.loops.pop();
    }

    fn visit_for_statement(&mut self, it: &ForStatement<'a>) {
        self.loops.push(range_of(it.span));
        walk::walk_for_statement(self, it);
        self.loops.pop();
    }

    fn visit_for_in_statement(&mut self, it: &ForInStatement<'a>) {
        self.loops.push(range_of(it.span));
        walk::walk_for_in_statement(self, it);
        self.loops.pop();
    }

    fn visit_while_statement(&mut self, it: &WhileStatement<'a>) {
        self.loops.push(range_of(it.span));
        walk::walk_while_statement(self, it);
        self.loops.pop();
    }

    fn visit_do_while_statement(&mut self, it: &DoWhileStatement<'a>) {
        self.loops.push(range_of(it.span));
        walk::walk_do_while_statement(self, it);
        self.loops.pop();
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
    let semantic = SemanticBuilder::new()
        .with_build_nodes(true)
        .build(&parsed.program)
        .semantic;
    let scoping = semantic.scoping();
    let mut writes = vec![Vec::new(); scoping.symbols_len()];
    for symbol in scoping.symbol_ids() {
        for reference in scoping.get_resolved_reference_ids(symbol) {
            let r = scoping.get_reference(*reference);
            if r.is_write() && !r.is_type() {
                writes[symbol.index()]
                    .push(range_of(semantic.nodes().get_node(r.node_id()).span()).0);
            }
        }
    }
    let mut visitor = Visitor {
        semantic: &semantic,
        input,
        lines: Lines::new(input.content),
        test_file: is_test_path(input.file),
        frames: Vec::new(),
        loops: Vec::new(),
        inherited: BTreeSet::new(),
        immediate: BTreeSet::new(),
        contextual: BTreeSet::new(),
        heads: BTreeMap::new(),
        writes,
        hits: Vec::new(),
    };
    visitor.visit_program(&parsed.program);
    one_per_line(visitor.hits)
}
