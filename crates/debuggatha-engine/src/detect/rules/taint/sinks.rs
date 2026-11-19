//! Where tainted data becomes an injection: shell commands, SQL text and Mongo filters.

use super::ast::{first_argument, has_shell_option, peel_ts_wrappers, segments};
use super::model::{Certainty, Imported};
use super::pass::Pass;
use super::vocabulary::{
    CONDITIONS, DRIVERS, INJECTION, LAYER_SQL, MONGO_METHODS, PROGRAM_FUNCTIONS, SHELL_FUNCTIONS,
    UNSAFE_BY_NAME, is_class_like, is_layer, lower,
};
use debuggatha_core::review_engine::Confidence;
use oxc_ast::ast::{Argument, CallExpression, Expression};

impl<'a> Pass<'a, '_, '_> {
    /// Is the call one that runs the shell, and what does it run?
    pub(super) fn shell_sink(&self, c: &CallExpression<'_>) -> Option<Certainty> {
        let function = match peel_ts_wrappers(&c.callee) {
            Expression::Identifier(id) => {
                match self.symbol(id).and_then(|s| self.state.shell.get(&s)) {
                    Some(Imported::Named(name)) => name.clone(),
                    _ => return None,
                }
            }
            Expression::StaticMemberExpression(m) => match peel_ts_wrappers(&m.object) {
                Expression::Identifier(id)
                    if matches!(
                        self.symbol(id).and_then(|s| self.state.shell.get(&s)),
                        Some(Imported::Whole)
                    ) =>
                {
                    m.property.name.to_string()
                }
                _ => return None,
            },
            _ => return None,
        };
        let function = function.as_str();
        let command = first_argument(c).and_then(|a| self.taint(a));
        if SHELL_FUNCTIONS.contains(&function) || PROGRAM_FUNCTIONS.contains(&function) {
            if command.is_some() {
                return command;
            }
            // `spawn('ls', [x], { shell: true })`: the arguments go through a shell.
            if PROGRAM_FUNCTIONS.contains(&function) && has_shell_option(c) {
                return c
                    .arguments
                    .get(1)
                    .and_then(Argument::as_expression)
                    .and_then(|a| self.taint(a));
            }
        }
        None
    }

    /// SQL text built from client data.
    pub(super) fn sql_sink(&self, c: &CallExpression<'_>) -> Option<Certainty> {
        let Expression::StaticMemberExpression(m) = peel_ts_wrappers(&c.callee) else {
            return None;
        };
        let method = m.property.name.as_str();
        let first = first_argument(c)?;
        if UNSAFE_BY_NAME.contains(&method) {
            return self.taint(first);
        }
        if CONDITIONS.contains(&method) {
            return match peel_ts_wrappers(first) {
                Expression::TemplateLiteral(_) | Expression::BinaryExpression(_) => {
                    self.taint(first)
                }
                _ => None,
            };
        }
        if LAYER_SQL.contains(&method) {
            let mut parts = Vec::new();
            segments(&m.object, &mut parts);
            if parts
                .iter()
                .any(|r| is_layer(r) || DRIVERS.contains(&lower(r).as_str()))
            {
                return self.taint(first);
            }
        }
        None
    }

    /// `User.find(req.query)`: the client picks the operators.
    pub(super) fn mongo_sink(&self, c: &CallExpression<'_>) -> bool {
        if self.state.mongo_sanitized {
            return false;
        }
        let Expression::StaticMemberExpression(m) = peel_ts_wrappers(&c.callee) else {
            return false;
        };
        if !MONGO_METHODS.contains(&m.property.name.as_str()) {
            return false;
        }
        let mut parts = Vec::new();
        segments(&m.object, &mut parts);
        let receiver = parts.first().is_some_and(|r| is_class_like(r))
            || parts
                .iter()
                .any(|r| is_layer(r) || lower(r).contains("collection"));
        receiver && first_argument(c).is_some_and(|a| self.is_whole_object(a))
    }

    pub(super) fn sinks(&mut self, c: &CallExpression<'a>) {
        let (start, end) = (c.span.start as usize, c.span.end as usize);
        if let Some(certainty) = self.sql_sink(c).or_else(|| self.shell_sink(c)) {
            self.injection(start, end, certainty);
        } else if self.mongo_sink(c) {
            self.hit(INJECTION, start, end, Confidence::Low);
        }
        // `validator.parse(req.body)`: the handler validates what it reads.
        if self.is_validator_call(c) {
            let takes_request = c
                .arguments
                .iter()
                .filter_map(Argument::as_expression)
                .any(|a| {
                    self.taint(a).is_some()
                        || matches!(peel_ts_wrappers(a), Expression::Identifier(id) if self.root(id).is_some())
                });
            if takes_request {
                for h in &mut self.state.handlers {
                    if h.start <= start && end <= h.end {
                        h.validated = true;
                    }
                }
            }
        }
        self.registration(c);
    }
}
