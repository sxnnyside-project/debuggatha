//! `no-floating-promises`, the part of it a syntax tree can prove: a statement whose dropped value
//! the tree shows to be a promise. Without types, nothing else is reported.
//!
//! - **High**: a call to an `async` function (or one declaring `: Promise<T>`) written in the same
//!   file, imported from a file of the review (`modules::ModuleIndex`) or a method of the enclosing
//!   class; `Promise.*`, `fetch`, `new Promise`, `import()` or an `async` function called on the
//!   spot; or a `.then`/`.finally` chain over one of those with no way to catch a rejection.
//! - **Low**: `x.then(callback)` with no rejection handler where nothing shows what `x` is (a
//!   Cypress chain or a query builder looks the same).
//!
//! Handled, so not reported: `await`, `void`, `return`, an assignment, an argument, `.catch(f)`,
//! `.then(f, g)`. A call the tree cannot tie to a promise (another object's method, a package, a
//! path alias, a file outside the review) needs types: the coverage report lists the rule as partial.

use std::collections::BTreeSet;

use oxc_allocator::Allocator;
use oxc_ast::AstKind;
use oxc_ast::ast::{
    CallExpression, ChainElement, Class, ClassElement, Expression, ExpressionStatement, Function,
    MethodDefinition, NewExpression, PropertyKey,
};
use oxc_ast_visit::{Visit, walk};
use oxc_parser::Parser;
use oxc_semantic::{ScopeFlags, Semantic, SemanticBuilder};

use super::calls::one_per_line;
use super::functions::peel_parens;
use super::hit::Hit;
use super::input::DetectorInput;
use super::js::{Lines, make_hit};
use super::modules::{
    ExportName, PROMISE_STATICS, expression_returns_a_promise, returns_a_promise,
};
use super::runner::Registry;
use crate::syntax::Language;
use crate::syntax::oxc::source_type;
use debuggatha_core::review_engine::{Confidence, Severity};

/// The rules this detector answers for.
pub const RULES: &[&str] = &["no-floating-promises"];

/// Registers the detector.
pub fn register(registry: &mut Registry) {
    registry.register_group(RULES, detect);
    registry.mark_partial(
        RULES[0],
        "only promises the syntax shows are checked (calls to async functions of the review, Promise.*, fetch, new Promise, import(), then-chains); a promise that comes from a method of another object, a package or a path alias needs types",
    );
}

/// What a name stands for, from the file's own declarations and imports.
#[derive(Clone, Default)]
struct Binding {
    /// A function that returns a promise.
    promising: bool,
    /// `import * as ns from 'specifier'`
    namespace: Option<String>,
}

struct Floating<'a, 's> {
    semantic: &'a Semantic<'a>,
    input: &'a DetectorInput<'a>,
    lines: Lines<'s>,
    bindings: Vec<Binding>,
    /// For each class being visited, the instance methods that return a promise. `None` where
    /// `this` is not that class (a plain function, an object method).
    classes: Vec<Option<BTreeSet<String>>>,
    /// Start offsets of functions that are methods of a class.
    methods: BTreeSet<u32>,
    hits: Vec<(&'static str, Hit, usize)>,
}

/// The expression without the wrappers that change nothing at run time.
fn peel<'e, 'a>(e: &'e Expression<'a>) -> &'e Expression<'a> {
    match peel_parens(e) {
        Expression::TSAsExpression(x) => peel(&x.expression),
        Expression::TSSatisfiesExpression(x) => peel(&x.expression),
        Expression::TSNonNullExpression(x) => peel(&x.expression),
        Expression::TSTypeAssertion(x) => peel(&x.expression),
        other => other,
    }
}

fn max_confidence(a: Option<Confidence>, b: Option<Confidence>) -> Option<Confidence> {
    match (a, b) {
        (Some(Confidence::High), _) | (_, Some(Confidence::High)) => Some(Confidence::High),
        (Some(x), _) | (_, Some(x)) => Some(x),
        _ => None,
    }
}

fn key_name<'a>(key: &'a PropertyKey<'_>) -> Option<&'a str> {
    match key {
        PropertyKey::StaticIdentifier(id) => Some(id.name.as_str()),
        _ => None,
    }
}

/// Instance members of a class that are functions returning a promise.
fn promising_members(class: &Class<'_>) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    for element in &class.body.body {
        match element {
            ClassElement::MethodDefinition(m)
                if !m.r#static
                    && m.kind == oxc_ast::ast::MethodDefinitionKind::Method
                    && returns_a_promise(&m.value) =>
            {
                names.extend(key_name(&m.key).map(str::to_owned));
            }
            ClassElement::PropertyDefinition(p)
                if !p.r#static && p.value.as_ref().is_some_and(expression_returns_a_promise) =>
            {
                names.extend(key_name(&p.key).map(str::to_owned));
            }
            _ => {}
        }
    }
    names
}

impl Floating<'_, '_> {
    fn symbol_of(&self, id: &oxc_ast::ast::IdentifierReference<'_>) -> Option<usize> {
        self.semantic
            .scoping()
            .get_reference(id.reference_id())
            .symbol_id()
            .map(oxc_semantic::SymbolId::index)
    }

    /// `Promise` or `fetch` as the language gives them, not a local of the same name.
    fn is_global(&self, id: &oxc_ast::ast::IdentifierReference<'_>, name: &str) -> bool {
        id.name == name && self.symbol_of(id).is_none()
    }

    fn is_promise_constructor(&self, e: &Expression<'_>) -> bool {
        matches!(peel(e), Expression::Identifier(id) if self.is_global(id, "Promise"))
    }

    /// Does this call return a promise, going by what the tree shows?
    fn returns_promise(&self, call: &CallExpression<'_>) -> bool {
        match peel(&call.callee) {
            Expression::Identifier(id) => match self.symbol_of(id) {
                Some(symbol) => self.bindings[symbol].promising,
                None => id.name == "fetch",
            },
            Expression::StaticMemberExpression(m) => {
                let property = m.property.name.as_str();
                match peel(&m.object) {
                    Expression::Identifier(object) => {
                        if self.is_global(object, "Promise") {
                            return PROMISE_STATICS.contains(&property);
                        }
                        self.symbol_of(object)
                            .and_then(|s| self.bindings[s].namespace.as_deref())
                            .is_some_and(|specifier| {
                                self.input.modules.is_some_and(|index| {
                                    index.export_returns_a_promise(
                                        self.input.file,
                                        specifier,
                                        ExportName::Named(property),
                                    )
                                })
                            })
                    }
                    Expression::ThisExpression(_) => self
                        .classes
                        .last()
                        .and_then(Option::as_ref)
                        .is_some_and(|names| names.contains(property)),
                    _ => false,
                }
            }
            // `(async () => { .. })()`
            callee @ (Expression::FunctionExpression(_)
            | Expression::ArrowFunctionExpression(_)) => expression_returns_a_promise(callee),
            _ => false,
        }
    }

    /// Is the expression certainly a promise?
    fn is_promise(&self, e: &Expression<'_>) -> bool {
        match peel(e) {
            Expression::CallExpression(call) => {
                if let Expression::StaticMemberExpression(m) = peel(&call.callee)
                    && matches!(m.property.name.as_str(), "then" | "catch" | "finally")
                {
                    return self.is_promise(&m.object);
                }
                self.returns_promise(call)
            }
            Expression::NewExpression(new) => self.is_promise_constructor(&new.callee),
            Expression::ImportExpression(_) => true,
            _ => false,
        }
    }

    fn call_floats(&self, call: &CallExpression<'_>) -> Option<Confidence> {
        if let Expression::StaticMemberExpression(m) = peel(&call.callee) {
            let arguments = call.arguments.len();
            match m.property.name.as_str() {
                // A rejection handler: nothing floats.
                "catch" if arguments >= 1 => return None,
                "then" if arguments >= 2 => return None,
                "then" | "catch" | "finally" => {
                    if self.is_promise(&m.object) {
                        return Some(Confidence::High);
                    }
                    let unverified_then = m.property.name == "then" && arguments == 1;
                    return unverified_then.then_some(Confidence::Low);
                }
                _ => {}
            }
        }
        self.returns_promise(call).then_some(Confidence::High)
    }

    fn new_floats(&self, new: &NewExpression<'_>) -> Option<Confidence> {
        self.is_promise_constructor(&new.callee)
            .then_some(Confidence::High)
    }

    /// The confidence of a floating promise in a value that is thrown away, if there is one.
    fn discarded(&self, e: &Expression<'_>) -> Option<Confidence> {
        match peel(e) {
            Expression::SequenceExpression(s) => {
                s.expressions.last().and_then(|x| self.discarded(x))
            }
            Expression::ConditionalExpression(c) => {
                max_confidence(self.discarded(&c.consequent), self.discarded(&c.alternate))
            }
            Expression::LogicalExpression(l) => {
                max_confidence(self.discarded(&l.left), self.discarded(&l.right))
            }
            Expression::ChainExpression(c) => match &c.expression {
                ChainElement::CallExpression(call) => self.call_floats(call),
                _ => None,
            },
            Expression::CallExpression(call) => self.call_floats(call),
            Expression::NewExpression(new) => self.new_floats(new),
            Expression::ImportExpression(_) => Some(Confidence::High),
            _ => None,
        }
    }
}

impl<'a> Visit<'a> for Floating<'a, '_> {
    fn visit_class(&mut self, it: &Class<'a>) {
        self.classes.push(Some(promising_members(it)));
        walk::walk_class(self, it);
        self.classes.pop();
    }

    fn visit_method_definition(&mut self, it: &MethodDefinition<'a>) {
        self.methods.insert(it.value.span.start);
        walk::walk_method_definition(self, it);
    }

    fn visit_function(&mut self, it: &Function<'a>, flags: ScopeFlags) {
        // `this` in a plain function is not the class; in a method it is.
        let is_method = self.methods.contains(&it.span.start);
        if !is_method {
            self.classes.push(None);
        }
        walk::walk_function(self, it, flags);
        if !is_method {
            self.classes.pop();
        }
    }

    fn visit_expression_statement(&mut self, it: &ExpressionStatement<'a>) {
        if let Some(confidence) = self.discarded(&it.expression) {
            let (start, end) = (it.span.start as usize, it.span.end as usize);
            if let Some(mut hit) = make_hit(self.input, &self.lines, start, end, confidence, None) {
                // A thenable of unverified origin is a suspicion: it must not fail a run the way a
                // proven floating promise (the rule's `critical`) does.
                if confidence == Confidence::Low {
                    hit.severity = Some(Severity::Medium);
                }
                self.hits.push((RULES[0], hit, start));
            }
        }
        walk::walk_expression_statement(self, it);
    }
}

/// What each symbol of the file stands for: a function that returns a promise, or a namespace of
/// another file.
fn bindings_of(semantic: &Semantic<'_>, input: &DetectorInput<'_>) -> Vec<Binding> {
    let scoping = semantic.scoping();
    let mut bindings = vec![Binding::default(); scoping.symbols_len()];
    for symbol in scoping.symbol_ids() {
        let node = semantic
            .nodes()
            .get_node(scoping.symbol_declaration(symbol));
        let reassigned = scoping
            .get_resolved_reference_ids(symbol)
            .iter()
            .any(|r| scoping.get_reference(*r).is_write());
        let binding = &mut bindings[symbol.index()];
        match node.kind() {
            AstKind::Function(f) => binding.promising = returns_a_promise(f),
            AstKind::VariableDeclarator(d) => {
                binding.promising =
                    !reassigned && d.init.as_ref().is_some_and(expression_returns_a_promise);
            }
            AstKind::ImportSpecifier(s) => {
                if let AstKind::ImportDeclaration(declaration) =
                    semantic.nodes().parent_kind(node.id())
                {
                    let imported = s.imported.name();
                    binding.promising = input.modules.is_some_and(|index| {
                        index.export_returns_a_promise(
                            input.file,
                            &declaration.source.value.display().to_string(),
                            if imported == "default" {
                                ExportName::Default
                            } else {
                                ExportName::Named(imported.as_str())
                            },
                        )
                    });
                }
            }
            AstKind::ImportDefaultSpecifier(_) => {
                if let AstKind::ImportDeclaration(declaration) =
                    semantic.nodes().parent_kind(node.id())
                {
                    binding.promising = input.modules.is_some_and(|index| {
                        index.export_returns_a_promise(
                            input.file,
                            &declaration.source.value.display().to_string(),
                            ExportName::Default,
                        )
                    });
                }
            }
            AstKind::ImportNamespaceSpecifier(_) => {
                if let AstKind::ImportDeclaration(declaration) =
                    semantic.nodes().parent_kind(node.id())
                {
                    binding.namespace = Some(declaration.source.value.display().to_string());
                }
            }
            _ => {}
        }
    }
    bindings
}

/// Runs the detector over one file.
#[must_use]
pub fn detect(input: &DetectorInput<'_>) -> Vec<(&'static str, Hit)> {
    let Some(syntax) = input.syntax else {
        return Vec::new();
    };
    if !matches!(
        syntax.language,
        Language::JavaScript | Language::Jsx | Language::TypeScript | Language::Tsx
    ) {
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
    let mut visitor = Floating {
        bindings: bindings_of(&semantic, input),
        semantic: &semantic,
        input,
        lines: Lines::new(input.content),
        classes: Vec::new(),
        methods: BTreeSet::new(),
        hits: Vec::new(),
    };
    visitor.visit_program(&parsed.program);
    one_per_line(visitor.hits)
}
