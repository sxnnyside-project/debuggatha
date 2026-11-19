//! The review domain of Debuggatha: what a review is, what a finding is, and how findings are
//! remembered. It knows nothing about adapters, parsers or the network, so everything here works
//! with no model and is tested without a process.
//!
//! | Module | Holds |
//! |---|---|
//! | [`repository_intelligence`] | Stack, capabilities, dependencies, documentation and criteria of a repository, as one immutable `RepositoryContext` |
//! | [`review_engine`] | A review request, its session and lifecycle, the [`Finding`](review_engine::Finding) and the result |
//! | [`knowledge_system`] | Skills, Review Packs, rules and the deterministic assembly of a `ReviewPolicy` |
//! | [`findings_ledger`] | The persistent history of findings: identity, five-state lifecycle, matching and synchronization |
//! | [`repository_memory`] | Long-term engineering decisions that may accept a finding |
//!
//! The clock and the id source are injected through [`env::Env`]; files are written atomically and
//! fail closed on a schema version they do not know ([`stored`]).

#![warn(missing_docs)]

pub mod canonical;
pub mod contract;
pub mod env;
pub mod findings_ledger;
pub mod js;
pub mod knowledge_system;
pub mod repository_intelligence;
pub mod repository_memory;
pub mod review_engine;
pub mod stored;
