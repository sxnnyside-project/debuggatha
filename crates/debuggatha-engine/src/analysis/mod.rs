//! The analysis engine. It discovers, it does not judge: module boundaries, the dependency graph
//! with real import resolution, layers, cycles, public API, dead code, ownership and change
//! impact. Every stage is a function of a [`Workspace`].
//!
//! Anything an import cannot be resolved to is recorded as unresolved with a reason, never guessed.

pub mod boundaries;
pub mod change_impact;
pub mod cycles;
pub mod dead_code;
pub mod declared;
pub mod ecosystem;
pub mod exports;
pub mod graph;
pub mod imports;
pub mod jsonc;
pub mod layers;
pub mod ownership;
pub mod public_api;
pub mod resolve;
pub mod run;
pub mod rust;
pub mod rust_api;
pub mod workspace;

pub use boundaries::{Boundaries, Manifest, ModuleBoundary, ModuleKind, infer_boundaries};
pub use change_impact::{
    ChangeImpact, ImpactConfidence, estimate_change_impact, estimate_change_impact_of_diff,
};
pub use cycles::{DependencyCycle, Impact, detect_cycles};
pub use dead_code::{Confidence, DeadCodeFinding, DeadCodeKind, detect_dead_code};
pub use declared::{build_declared_public_api, detect_declared_dead_code};
pub use exports::{ExportForm, ExportGraph, Facts, FileFacts, read_all, read_facts};
pub use graph::{
    DependencyEdge, DependencyGraph, EdgeKind, Evidence, UnresolvedImport, build_graph,
};
pub use layers::{
    LAYER_ORDER, LayerModel, LayerName, LayerViolation, build_layer_model, detect_layer_violations,
};
pub use ownership::{OwnershipSignal, OwnershipSource, detect_ownership};
pub use public_api::{
    PublicApiSurface, PublicApiSymbol, Stability, build_public_api, loaded_by_convention,
};
pub use resolve::{Resolution, Resolver, Unresolved};
pub use run::{Analysis, NotCovered, analyze};
pub use workspace::Workspace;
