//! What the analysis tracks: how sure it is, where a value came from, and what it has seen.

use crate::detect::hit::Hit;
use oxc_semantic::SymbolId;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub(super) enum Certainty {
    /// Reaches the sink through a variable assigned more than once.
    Maybe,
    /// Every link of the flow is a single binding.
    Sure,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Root {
    /// `req`, `request`.
    Req,
    /// `ctx`, `context`.
    Ctx,
    /// `c`.
    Hono,
}

pub(super) fn root_of(name: &str) -> Option<Root> {
    match name {
        "req" | "request" => Some(Root::Req),
        "ctx" | "context" => Some(Root::Ctx),
        "c" => Some(Root::Hono),
        _ => None,
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Prop {
    Body,
    Query,
}

pub(super) struct Handler {
    pub(super) start: usize,
    pub(super) end: usize,
    pub(super) reads: Vec<(Prop, usize, usize)>,
    /// A validator was called on request data inside.
    pub(super) validated: bool,
    /// Registered in this file with a route call.
    pub(super) registered: bool,
    /// Registered behind a validator.
    pub(super) upstream: bool,
}

/// What a name imported from `child_process` or `shelljs` stands for.
#[derive(Clone)]
pub(super) enum Imported {
    /// `import cp from`, `import * as cp from`, `const cp = require(..)`.
    Whole,
    Named(String),
}

#[derive(Default)]
pub(super) struct State {
    pub(super) request: BTreeMap<SymbolId, Root>,
    pub(super) handlers: Vec<Handler>,
    pub(super) handler_of: BTreeMap<SymbolId, usize>,
    pub(super) tainted: BTreeMap<SymbolId, Certainty>,
    /// Bound to a request object as a whole (`const filter = req.query`).
    pub(super) whole: BTreeSet<SymbolId>,
    pub(super) validators: BTreeSet<SymbolId>,
    pub(super) shell: BTreeMap<SymbolId, Imported>,
    pub(super) mongo_sanitized: bool,
    /// An `app.use(validate)` somewhere: every route is behind it.
    pub(super) global_validation: bool,
    /// Spans of logging calls, whose arguments are not a use of the data.
    pub(super) logging: Vec<(usize, usize)>,
    pub(super) hits: Vec<(&'static str, Hit, usize)>,
    pub(super) changed: bool,
}

pub(super) fn best(items: impl IntoIterator<Item = Option<Certainty>>) -> Option<Certainty> {
    items.into_iter().flatten().max()
}
