//! The review engine: skills, analysis, analyzers, baseline and scan scope.

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
