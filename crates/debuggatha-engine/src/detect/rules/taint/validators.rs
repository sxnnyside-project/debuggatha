//! Which calls validate input, and which handlers register without one.

use super::ast::{peel_ts_wrappers, segments};
use super::pass::Pass;
use super::vocabulary::{NOT_A_SCHEMA, VALIDATING, VERBS, is_validator_name};
use oxc_ast::AstKind;
use oxc_ast::ast::{
    Argument, CallExpression, Expression, Function, IdentifierName, IdentifierReference,
    LogicalOperator, StaticMemberExpression, UnaryOperator,
};
use oxc_ast_visit::Visit;
use oxc_semantic::{ScopeFlags, Semantic, SymbolId};
use std::collections::BTreeSet;

/// Finds a validator among the names an expression mentions.
pub(super) struct FindValidator<'a, 'b> {
    pub(super) semantic: &'a Semantic<'a>,
    pub(super) validators: &'b BTreeSet<SymbolId>,
    pub(super) found: bool,
}

impl<'a> Visit<'a> for FindValidator<'a, '_> {
    fn visit_identifier_reference(&mut self, it: &IdentifierReference<'a>) {
        let symbol = self
            .semantic
            .scoping()
            .get_reference(it.reference_id())
            .symbol_id();
        if is_validator_name(&it.name) || symbol.is_some_and(|s| self.validators.contains(&s)) {
            self.found = true;
        }
    }

    fn visit_identifier_name(&mut self, it: &IdentifierName<'a>) {
        if is_validator_name(&it.name) {
            self.found = true;
        }
    }

    // A function among the arguments is the handler or a middleware, not a validator: what is
    // inside it says nothing about what stands before it.
    fn visit_function(&mut self, _it: &Function<'a>, _flags: ScopeFlags) {}

    fn visit_arrow_function_expression(&mut self, _it: &oxc_ast::ast::ArrowFunctionExpression<'a>) {
    }
}

impl<'a> Pass<'a, '_, '_> {
    pub(super) fn is_validator_call(&self, c: &CallExpression<'_>) -> bool {
        let mut finder = FindValidator {
            semantic: self.semantic,
            validators: &self.state.validators,
            found: false,
        };
        match peel_ts_wrappers(&c.callee) {
            Expression::StaticMemberExpression(m) => {
                let mut parts = Vec::new();
                segments(&m.object, &mut parts);
                let root = parts.first().map_or("", String::as_str);
                if VALIDATING.contains(&m.property.name.as_str()) && !NOT_A_SCHEMA.contains(&root) {
                    return true;
                }
                finder.visit_expression(&c.callee);
            }
            callee => finder.visit_expression(callee),
        }
        finder.found
    }

    /// Is the expression, or a name inside it, a validator?
    pub(super) fn mentions_validator(&self, e: &Expression<'a>) -> bool {
        let mut finder = FindValidator {
            semantic: self.semantic,
            validators: &self.state.validators,
            found: false,
        };
        finder.visit_expression(e);
        finder.found
    }

    /// Marks the handlers a route call registers, and whether a validator stands before them.
    pub(super) fn registration(&mut self, c: &CallExpression<'a>) {
        let Expression::StaticMemberExpression(m) = peel_ts_wrappers(&c.callee) else {
            return;
        };
        let verb = m.property.name.as_str();
        if !VERBS.contains(&verb) {
            return;
        }
        let expressions: Vec<&Expression<'a>> = c
            .arguments
            .iter()
            .filter_map(Argument::as_expression)
            .collect();
        let validators: Vec<bool> = expressions
            .iter()
            .map(|e| self.mentions_validator(e))
            .collect();
        if verb == "use" && validators.iter().any(|v| *v) {
            self.state.global_validation = true;
        }
        for (i, e) in expressions.iter().enumerate() {
            let (start, end) = (
                oxc_span::GetSpan::span(*e).start as usize,
                oxc_span::GetSpan::span(*e).end as usize,
            );
            let mut starts: Vec<usize> = Vec::new();
            if let Expression::Identifier(id) = peel_ts_wrappers(e) {
                if let Some(s) = self.function_start(id) {
                    starts.push(s);
                }
            } else {
                starts.extend(
                    self.state
                        .handlers
                        .iter()
                        .filter(|h| h.start >= start && h.end <= end)
                        .map(|h| h.start),
                );
            }
            let upstream = validators[..i].iter().any(|v| *v)
                || matches!(peel_ts_wrappers(e), Expression::CallExpression(call) if self.is_validator_call(call));
            for h in &mut self.state.handlers {
                if starts.contains(&h.start) {
                    h.registered = true;
                    h.upstream |= upstream;
                }
            }
        }
    }

    /// `!req.body`, `typeof req.body`, `if (req.body)`, `req.body && ..`: asking whether the data
    /// is there is not trusting it.
    pub(super) fn is_presence_check(&self, it: &StaticMemberExpression<'_>) -> bool {
        let span = it.span;
        match self.semantic.nodes().parent_kind(it.node_id.get()) {
            AstKind::UnaryExpression(u) => {
                matches!(
                    u.operator,
                    UnaryOperator::LogicalNot | UnaryOperator::Typeof
                )
            }
            AstKind::IfStatement(i) => oxc_span::GetSpan::span(&i.test) == span,
            AstKind::LogicalExpression(l) => {
                l.operator != LogicalOperator::Coalesce && oxc_span::GetSpan::span(&l.left) == span
            }
            _ => false,
        }
    }
}
