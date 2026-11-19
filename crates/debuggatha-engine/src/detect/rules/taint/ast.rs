//! Small readings of the syntax tree the analysis shares.

use crate::detect::rules::js::peel;
use oxc_ast::ast::{
    Argument, BindingPattern, CallExpression, ChainElement, Expression, ObjectExpression,
    ObjectPropertyKind, PropertyKey,
};
use oxc_semantic::SymbolId;

/// The names along a receiver chain, root first: `this.db.user.findMany` is
/// `["this", "db", "user", "findMany"]`; `knex('t').where(..)` is `["knex", "where"]`.
pub(super) fn segments(e: &Expression<'_>, out: &mut Vec<String>) {
    match peel(e) {
        Expression::Identifier(id) => out.push(id.name.to_string()),
        Expression::StaticMemberExpression(m) => {
            segments(&m.object, out);
            out.push(m.property.name.to_string());
        }
        Expression::ComputedMemberExpression(m) => segments(&m.object, out),
        Expression::CallExpression(c) => segments(&c.callee, out),
        Expression::TSNonNullExpression(x) => segments(&x.expression, out),
        Expression::ChainExpression(c) => {
            if let ChainElement::CallExpression(call) = &c.expression {
                segments(&call.callee, out);
            }
        }
        _ => {}
    }
}

pub(super) fn peel_ts_wrappers<'e, 'a>(mut e: &'e Expression<'a>) -> &'e Expression<'a> {
    loop {
        e = match peel(e) {
            Expression::TSAsExpression(x) => &x.expression,
            Expression::TSSatisfiesExpression(x) => &x.expression,
            Expression::TSNonNullExpression(x) => &x.expression,
            Expression::TSTypeAssertion(x) => &x.expression,
            other => return other,
        };
    }
}

pub(super) fn bindings(pattern: &BindingPattern<'_>, out: &mut Vec<SymbolId>) {
    match pattern {
        BindingPattern::BindingIdentifier(id) => out.extend(id.symbol_id.get()),
        BindingPattern::ObjectPattern(o) => {
            for p in &o.properties {
                bindings(&p.value, out);
            }
            if let Some(rest) = &o.rest {
                bindings(&rest.argument, out);
            }
        }
        BindingPattern::ArrayPattern(a) => {
            for element in a.elements.iter().flatten() {
                bindings(element, out);
            }
            if let Some(rest) = &a.rest {
                bindings(&rest.argument, out);
            }
        }
        BindingPattern::AssignmentPattern(a) => bindings(&a.left, out),
    }
}

pub(super) fn key_name(key: &PropertyKey<'_>) -> Option<String> {
    match key {
        PropertyKey::StaticIdentifier(id) => Some(id.name.to_string()),
        PropertyKey::StringLiteral(s) => Some(s.value.display().to_string()),
        _ => None,
    }
}

pub(super) fn first_argument<'c, 'b>(c: &'c CallExpression<'b>) -> Option<&'c Expression<'b>> {
    c.arguments.first().and_then(Argument::as_expression)
}

pub(super) fn has_shell_option(c: &CallExpression<'_>) -> bool {
    c.arguments
        .iter()
        .filter_map(Argument::as_expression)
        .any(|a| match peel_ts_wrappers(a) {
            Expression::ObjectExpression(o) => option_is_true(o, "shell"),
            _ => false,
        })
}

pub(super) fn option_is_true(o: &ObjectExpression<'_>, name: &str) -> bool {
    o.properties.iter().any(|p| match p {
        ObjectPropertyKind::ObjectProperty(p) => {
            key_name(&p.key).as_deref() == Some(name)
                && matches!(&p.value, Expression::BooleanLiteral(b) if b.value)
        }
        ObjectPropertyKind::SpreadProperty(_) => false,
    })
}
