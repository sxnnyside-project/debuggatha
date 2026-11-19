//! Whether untrusted input reaches a span, inside one file: flow-insensitive and by name, to a
//! fixed point. A source is a call or a path the `untrusted-input` concept binds, or a parameter
//! named like one; a value passes through assignments and expressions, and stops at a cast or at a
//! call the `sanitizer` concept binds. A call to a function the file does not define is not
//! followed.

use super::{Concepts, Model};
use crate::syntax::{Language, Span};

/// How sure a flow is: `Uncertain` when it goes through a name assigned more than once, since the
/// analysis does not know which assignment reaches the use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Certainty {
    /// The flow goes through a name assigned more than once.
    Uncertain,
    /// The flow does not.
    Certain,
}

/// Whether untrusted input reaches a span, within one file.
pub struct Flow<'m> {
    model: &'m Model,
    sources: Vec<Span>,
    cleaners: Vec<Span>,
    repeated: Vec<bool>,
    state: Vec<Option<Certainty>>,
}

fn within(outer: Span, inner: Span) -> bool {
    outer.start <= inner.start && inner.end <= outer.end
}

impl<'m> Flow<'m> {
    /// The flow of untrusted input through `model`, to a fixed point.
    #[must_use]
    pub fn new(model: &'m Model, table: &Concepts, language: Language) -> Self {
        let is = |concept: &str, text: &str| table.is(concept, language, text);
        let resolved_variable = |span: Span| {
            model
                .reads
                .iter()
                .find(|(r, _)| *r == span)
                .is_some_and(|(_, i)| model.bindings[*i].value.is_some())
        };
        let mut sources: Vec<Span> = model
            .calls
            .iter()
            .filter(|c| is("untrusted-input", &c.callee))
            .map(|c| c.span)
            .collect();
        sources.extend(
            model
                .paths
                .iter()
                .filter(|p| is("untrusted-input", &p.text) && !resolved_variable(p.span))
                .map(|p| p.span),
        );
        let mut cleaners: Vec<Span> = model
            .calls
            .iter()
            .filter(|c| is("sanitizer", &c.callee))
            .filter_map(|c| c.args)
            .collect();
        cleaners.extend(&model.cleaned);
        let repeated = model
            .bindings
            .iter()
            .map(|b| {
                model
                    .bindings
                    .iter()
                    .filter(|o| o.name == b.name && o.function == b.function)
                    .count()
                    > 1
            })
            .collect();
        let state = model
            .bindings
            .iter()
            .map(|b| {
                (b.value.is_none() && b.loop_body.is_none() && is("untrusted-input", &b.name))
                    .then_some(Certainty::Certain)
            })
            .collect();
        let mut flow = Self {
            model,
            sources,
            cleaners,
            repeated,
            state,
        };
        flow.settle();
        flow
    }

    fn settle(&mut self) {
        loop {
            let mut changed = false;
            for (i, binding) in self.model.bindings.iter().enumerate() {
                let Some(value) = binding.value else {
                    continue;
                };
                let reached = self.reaches(value);
                if reached > self.state[i] {
                    self.state[i] = reached;
                    changed = true;
                }
            }
            if !changed {
                return;
            }
        }
    }

    fn cleaned(&self, span: Span) -> bool {
        self.cleaners.iter().any(|c| within(*c, span))
    }

    /// Does `span` read any name that no cast or cleaner covers? A string built only from fixed
    /// text and cleaned parts exposes nothing.
    #[must_use]
    pub fn exposes(&self, span: Span) -> bool {
        self.model
            .paths
            .iter()
            .any(|p| within(span, p.span) && !self.cleaned(p.span))
    }

    /// The strongest way untrusted input reaches `span`, if it does.
    #[must_use]
    pub fn reaches(&self, span: Span) -> Option<Certainty> {
        if self
            .sources
            .iter()
            .any(|s| within(span, *s) && !self.cleaned(*s))
        {
            return Some(Certainty::Certain);
        }
        self.model
            .reads
            .iter()
            .filter(|(r, _)| within(span, *r) && !self.cleaned(*r))
            .filter_map(|(_, i)| {
                let link = if self.repeated[*i] {
                    Certainty::Uncertain
                } else {
                    Certainty::Certain
                };
                self.state[*i].map(|s| s.min(link))
            })
            .max()
    }
}
