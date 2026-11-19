//! The review engine of Debuggatha: it runs a review over a repository and returns findings that
//! cite a real source, and it says what it could not check.
//!
//! [`pipeline::execute_review`] is the entry point: it orients on the repository, resolves the
//! policy of the matching Review Packs, runs the detectors, the external analyzers and the
//! optional model pass, applies suppressions, and records the findings in the ledger.
//!
//! | Module | Holds |
//! |---|---|
//! | [`syntax`] | One parse per file (oxc for JavaScript and TypeScript, tree-sitter for the rest) and its status |
//! | [`semantic`] | The language-neutral model detectors read: calls, bindings, declarations, data flow |
//! | [`detect`] | The detectors, and the coverage of what could not be checked |
//! | [`analysis`] | Module boundaries, dependency graph, layers, cycles, public API, dead code, ownership, change impact |
//! | [`analyzers`] | External tools run as separate processes |
//! | [`semantic_layer`] | The optional, grounded model pass |
//! | [`daemon`] | Protocol v1: the engine behind `debuggatha serve` and `debuggatha mcp` |
//! | [`baseline`], [`scan_scope`], [`diff`], [`git`] | What counts as new, what is owned, what changed |

#![warn(missing_docs)]

pub use debuggatha_core as core;

pub mod analysis;
pub mod analyzers;
pub mod baseline;
pub mod client_error;
pub mod control;
pub mod daemon;
pub mod detect;
pub mod diff;
pub mod git;
pub mod packs;
pub mod pipeline;
pub mod scan_scope;
pub mod semantic;
pub mod semantic_layer;
pub mod syntax;

/// The engine version, the same as the CLI's.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
