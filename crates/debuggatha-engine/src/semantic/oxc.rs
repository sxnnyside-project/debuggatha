//! The oxc adapter: the same relations the queries give for the other languages, read from the
//! JavaScript and TypeScript syntax tree. Names are resolved the way every adapter resolves them
//! (by name, inside the function and the ones around it), so a rule over the model reads one
//! language like any other.

use oxc_allocator::Allocator;
use oxc_ast::ast::{
    ArrowFunctionExpression, AssignmentExpression, AssignmentTarget, BinaryExpression,
    BinaryOperator, BindingPattern, CallExpression, CatchClause, DoWhileStatement, Expression,
    ForInStatement, ForOfStatement, ForStatement, ForStatementInit, ForStatementLeft, Function,
    IdentifierName, IdentifierReference, ImportDeclaration, ImportExpression, NewExpression,
    Program, Statement, TaggedTemplateExpression, TemplateLiteral, UnaryExpression, UnaryOperator,
    VariableDeclaration, VariableDeclarator, WhileStatement,
};
use oxc_ast::ast::{Declaration, ModuleDeclaration};
use oxc_ast_visit::{Visit, walk};
use oxc_parser::Parser;
use oxc_semantic::ScopeFlags;
use oxc_span::GetSpan;

use super::build::{Found, RawDecl, assemble, normalize};
use super::{Call, Model};
use crate::syntax::oxc::source_type;
use crate::syntax::{Language, Span, script_regions};

/// A `for` that runs this many times or fewer is a fixed small loop.
const SMALL_BOUND: f64 = 10.0;

fn peel<'a, 'b>(e: &'b Expression<'a>) -> &'b Expression<'a> {
    match e {
        Expression::ParenthesizedExpression(p) => peel(&p.expression),
        other => other,
    }
}

/// Calls that open a test: what they hold is test code.
const TEST_BLOCKS: &[&str] = &[
    "describe",
    "it",
    "test",
    "suite",
    "context",
    "specify",
    "beforeEach",
    "afterEach",
    "beforeAll",
    "afterAll",
];

fn span(s: oxc_span::Span) -> Span {
    Span::new(s.start, s.end)
}

struct Collector<'s> {
    source: &'s str,
    found: Found,
}

impl Collector<'_> {
    fn text(&self, s: oxc_span::Span) -> &str {
        self.source
            .get(s.start as usize..s.end as usize)
            .unwrap_or_default()
    }

    fn bind(&mut self, pattern: &BindingPattern<'_>, loop_body: Option<Span>, value: Option<Span>) {
        match pattern {
            BindingPattern::BindingIdentifier(id) => {
                self.found.defs.push((span(id.span), loop_body, value));
            }
            BindingPattern::ObjectPattern(o) => {
                for p in &o.properties {
                    self.bind(&p.value, loop_body, value);
                }
                if let Some(rest) = &o.rest {
                    self.bind(&rest.argument, loop_body, value);
                }
            }
            BindingPattern::ArrayPattern(a) => {
                for element in a.elements.iter().flatten() {
                    self.bind(element, loop_body, value);
                }
                if let Some(rest) = &a.rest {
                    self.bind(&rest.argument, loop_body, value);
                }
            }
            BindingPattern::AssignmentPattern(a) => self.bind(&a.left, loop_body, value),
        }
    }

    fn bind_declaration(&mut self, d: &VariableDeclaration<'_>, loop_body: Option<Span>) {
        for declarator in &d.declarations {
            self.bind(&declarator.id, loop_body, None);
        }
    }

    /// Is this argument built from interpolation or concatenation?
    fn built(argument: &Expression<'_>) -> bool {
        match argument {
            Expression::TemplateLiteral(t) => !t.expressions.is_empty(),
            Expression::BinaryExpression(b) => {
                b.operator == BinaryOperator::Addition
                    && !(matches!(b.left, Expression::StringLiteral(_))
                        && matches!(b.right, Expression::StringLiteral(_)))
            }
            Expression::ParenthesizedExpression(p) => Self::built(&p.expression),
            _ => false,
        }
    }

    fn open_paren(&self, from: u32) -> Option<u32> {
        let rest = self.source.get(from as usize..)?;
        Some(from + u32::try_from(rest.find('(')?).ok()?)
    }

    fn call(&mut self, it: &CallExpression<'_>) {
        let callee = it.callee.span();
        let from = it
            .type_arguments
            .as_ref()
            .map_or(callee.end, |t| t.span.end);
        let Some(open) = self.open_paren(from) else {
            return;
        };
        let mut arg_list = Vec::new();
        for argument in &it.arguments {
            let s = span(argument.span());
            arg_list.push(s);
            if argument.as_expression().is_some_and(Self::built) {
                self.found.dynamic.push(s);
            }
        }
        let name = normalize(self.text(callee));
        if TEST_BLOCKS.contains(&name.as_str()) {
            self.found.tests.push(span(it.span));
        }
        if let Expression::StaticMemberExpression(m) = &it.callee
            && matches!(peel(&m.object), Expression::ArrayExpression(_))
        {
            self.found.literal_iterations.push(span(it.span));
        }
        if matches!(it.callee, Expression::ComputedMemberExpression(_)) {
            self.found.dynamic_calls.push(span(it.span));
        }
        if name == "require"
            && let Some(Expression::StringLiteral(s)) =
                it.arguments.first().and_then(|a| a.as_expression())
        {
            self.found.imports.push(span(s.span));
        }
        self.found.calls.push(Call {
            span: span(it.span),
            callee: name,
            callee_span: span(callee),
            args: Some(Span::new(open, it.span.end)),
            arg_list,
            dynamic_args: Vec::new(),
            function: None,
            refs: Vec::new(),
            loop_depth: 0,
            in_test: false,
        });
    }

    fn function(&mut self, params: &[&BindingPattern<'_>], whole: oxc_span::Span) {
        self.found.functions.push(span(whole));
        for p in params {
            self.bind(p, None, None);
        }
    }
}

impl<'a> Visit<'a> for Collector<'_> {
    fn visit_call_expression(&mut self, it: &CallExpression<'a>) {
        self.call(it);
        walk::walk_call_expression(self, it);
    }

    fn visit_new_expression(&mut self, it: &NewExpression<'a>) {
        let callee = it.callee.span();
        if let Some(open) = self.open_paren(callee.end) {
            let mut arg_list = Vec::new();
            for argument in &it.arguments {
                let s = span(argument.span());
                arg_list.push(s);
                if argument.as_expression().is_some_and(Self::built) {
                    self.found.dynamic.push(s);
                }
            }
            self.found.calls.push(Call {
                span: span(it.span),
                callee: normalize(self.text(callee)),
                callee_span: span(callee),
                args: Some(Span::new(open, it.span.end)),
                arg_list,
                dynamic_args: Vec::new(),
                function: None,
                refs: Vec::new(),
                loop_depth: 0,
                in_test: false,
            });
        }
        walk::walk_new_expression(self, it);
    }

    fn visit_tagged_template_expression(&mut self, it: &TaggedTemplateExpression<'a>) {
        let quasi = span(it.quasi.span);
        let callee = it.tag.span();
        if !it.quasi.expressions.is_empty() {
            self.found.dynamic.push(quasi);
        }
        self.found.calls.push(Call {
            span: span(it.span),
            callee: normalize(self.text(callee)),
            callee_span: span(callee),
            args: Some(quasi),
            arg_list: vec![quasi],
            dynamic_args: Vec::new(),
            function: None,
            refs: Vec::new(),
            loop_depth: 0,
            in_test: false,
        });
        walk::walk_tagged_template_expression(self, it);
    }

    fn visit_catch_clause(&mut self, it: &CatchClause<'a>) {
        let inside = self
            .text(it.body.span)
            .trim_start_matches('{')
            .trim_end_matches('}');
        if inside.trim().is_empty() {
            self.found.empty_handlers.push(span(it.span));
        }
        walk::walk_catch_clause(self, it);
    }

    fn visit_function(&mut self, it: &Function<'a>, flags: ScopeFlags) {
        let params: Vec<&BindingPattern<'_>> = it.params.items.iter().map(|p| &p.pattern).collect();
        self.function(&params, it.span);
        walk::walk_function(self, it, flags);
    }

    fn visit_arrow_function_expression(&mut self, it: &ArrowFunctionExpression<'a>) {
        let params: Vec<&BindingPattern<'_>> = it.params.items.iter().map(|p| &p.pattern).collect();
        self.function(&params, it.span);
        walk::walk_arrow_function_expression(self, it);
    }

    fn visit_for_statement(&mut self, it: &ForStatement<'a>) {
        let body = span(it.body.span());
        self.found.loops.push(body);
        let small = matches!(
            it.test.as_ref().map(peel),
            Some(Expression::BinaryExpression(b))
                if matches!(&b.right, Expression::NumericLiteral(n) if n.value <= SMALL_BOUND)
        );
        if small {
            self.found.small_loops.push(body);
        }
        if let Some(ForStatementInit::VariableDeclaration(d)) = &it.init {
            self.bind_declaration(d, Some(body));
        }
        walk::walk_for_statement(self, it);
    }

    fn visit_for_in_statement(&mut self, it: &ForInStatement<'a>) {
        let body = span(it.body.span());
        self.found.loops.push(body);
        if let ForStatementLeft::VariableDeclaration(d) = &it.left {
            self.bind_declaration(d, Some(body));
        }
        walk::walk_for_in_statement(self, it);
    }

    fn visit_for_of_statement(&mut self, it: &ForOfStatement<'a>) {
        let body = span(it.body.span());
        self.found.loops.push(body);
        if matches!(peel(&it.right), Expression::ArrayExpression(_)) {
            self.found.small_loops.push(body);
        }
        if let ForStatementLeft::VariableDeclaration(d) = &it.left {
            self.bind_declaration(d, Some(body));
        }
        walk::walk_for_of_statement(self, it);
    }

    fn visit_while_statement(&mut self, it: &WhileStatement<'a>) {
        self.found.loops.push(span(it.body.span()));
        walk::walk_while_statement(self, it);
    }

    fn visit_do_while_statement(&mut self, it: &DoWhileStatement<'a>) {
        self.found.loops.push(span(it.body.span()));
        walk::walk_do_while_statement(self, it);
    }

    fn visit_variable_declarator(&mut self, it: &VariableDeclarator<'a>) {
        let value = it.init.as_ref().map(|i| span(i.span()));
        self.bind(&it.id, None, value);
        walk::walk_variable_declarator(self, it);
    }

    fn visit_assignment_expression(&mut self, it: &AssignmentExpression<'a>) {
        if let AssignmentTarget::AssignmentTargetIdentifier(id) = &it.left {
            self.found
                .defs
                .push((span(id.span), None, Some(span(it.right.span()))));
        }
        walk::walk_assignment_expression(self, it);
    }

    fn visit_identifier_reference(&mut self, it: &IdentifierReference<'a>) {
        self.found.refs.push(span(it.span));
        self.found.mentions.push(span(it.span));
    }

    fn visit_identifier_name(&mut self, it: &IdentifierName<'a>) {
        self.found.mentions.push(span(it.span));
    }

    fn visit_static_member_expression(&mut self, it: &oxc_ast::ast::StaticMemberExpression<'a>) {
        self.found.paths.push(span(it.span));
        walk::walk_static_member_expression(self, it);
    }

    fn visit_unary_expression(&mut self, it: &UnaryExpression<'a>) {
        if it.operator == UnaryOperator::UnaryPlus {
            self.found.cleaned.push(span(it.span));
        }
        walk::walk_unary_expression(self, it);
    }

    fn visit_binary_expression(&mut self, it: &BinaryExpression<'a>) {
        walk::walk_binary_expression(self, it);
    }

    fn visit_template_literal(&mut self, it: &TemplateLiteral<'a>) {
        walk::walk_template_literal(self, it);
    }

    fn visit_import_declaration(&mut self, it: &ImportDeclaration<'a>) {
        self.found.imports.push(span(it.source.span));
        walk::walk_import_declaration(self, it);
    }

    fn visit_import_expression(&mut self, it: &ImportExpression<'a>) {
        if let Expression::StringLiteral(s) = &it.source {
            self.found.imports.push(span(s.span));
        }
        walk::walk_import_expression(self, it);
    }
}

/// The top-level functions, classes and variables, public when an `export` carries them.
fn declare(collector: &mut Collector<'_>, program: &Program<'_>) {
    let mut add =
        |name: &oxc_ast::ast::BindingIdentifier<'_>, node: oxc_span::Span, exported: bool| {
            collector.found.decls.push(RawDecl {
                name: span(name.span),
                node: span(node),
                modifiers: Vec::new(),
                member: false,
                wired: false,
            });
            if exported {
                collector.found.exported.push(span(name.span));
            }
        };
    for statement in &program.body {
        match statement {
            Statement::FunctionDeclaration(f) => {
                if let Some(id) = &f.id {
                    add(id, f.span, false);
                }
            }
            Statement::ClassDeclaration(c) => {
                if let Some(id) = &c.id {
                    add(id, c.span, false);
                }
            }
            other => {
                if let Some(ModuleDeclaration::ExportDeclaration(e)) = other.as_module_declaration()
                {
                    match &e.declaration {
                        Declaration::FunctionDeclaration(f) => {
                            if let Some(id) = &f.id {
                                add(id, f.span, true);
                            }
                        }
                        Declaration::ClassDeclaration(c) => {
                            if let Some(id) = &c.id {
                                add(id, c.span, true);
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }
}

/// The model of a Vue, Svelte or Astro file: its scripts, read as one program at their own offsets
/// (everything outside them is blanked, newlines kept), or `None` when there is none or it does
/// not parse. Templates and styles hold no facts.
#[must_use]
pub(super) fn model_of_sfc(language: Language, source: &str) -> Option<Model> {
    let regions = script_regions(source, language);
    if regions.is_empty() {
        return None;
    }
    let masked: String = source
        .char_indices()
        .map(|(i, ch)| {
            if ch == '\n' || regions.iter().any(|r| (r.start..r.end).contains(&i)) {
                ch.to_string()
            } else {
                " ".repeat(ch.len_utf8())
            }
        })
        .collect();
    let typed = regions
        .iter()
        .any(|r| matches!(r.language, Language::TypeScript | Language::Tsx));
    model_of_js(
        if typed {
            Language::TypeScript
        } else {
            Language::Jsx
        },
        &masked,
    )
}

/// The model of a JavaScript or TypeScript file, or `None` when it does not parse.
#[must_use]
pub(super) fn model_of_js(language: Language, source: &str) -> Option<Model> {
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, source, source_type(language, false)).parse();
    if parsed.fatal_error {
        return None;
    }
    let mut collector = Collector {
        source,
        found: Found::default(),
    };
    collector.visit_program(&parsed.program);
    declare(&mut collector, &parsed.program);
    Some(assemble(collector.found, source, language, &[]))
}
