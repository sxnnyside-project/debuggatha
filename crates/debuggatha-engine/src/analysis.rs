//! The analysis engine. It discovers, it does not judge: module boundaries, the dependency graph
//! with real import resolution, layers, cycles, public API, dead code, ownership and change
//! impact. Every stage is a function of a [`Workspace`].
//!
//! Anything an import cannot be resolved to is recorded as unresolved with a reason, never guessed.

pub mod change_impact;
pub mod ecosystem;
pub mod jsonc;
pub mod ownership;
pub mod run;
pub mod structure;
pub mod surface;

pub use change_impact::{
    ChangeImpact, ImpactConfidence, estimate_change_impact, estimate_change_impact_of_diff,
};
pub use ownership::{OwnershipSignal, OwnershipSource, detect_ownership};
pub use run::{Analysis, NotCovered, analyze};
pub use structure::boundaries::{
    Boundaries, Manifest, ModuleBoundary, ModuleKind, infer_boundaries,
};
pub use structure::cycles::{DependencyCycle, Impact, detect_cycles};
pub use structure::graph::{
    DependencyEdge, DependencyGraph, EdgeKind, Evidence, UnresolvedImport, build_graph,
};
pub use structure::layers::{
    LAYER_ORDER, LayerModel, LayerName, LayerViolation, build_layer_model, detect_layer_violations,
};
pub use structure::resolve::{Resolution, Resolver, Unresolved};
pub use structure::workspace::Workspace;
pub use surface::dead_code::{Confidence, DeadCodeFinding, DeadCodeKind, detect_dead_code};
pub use surface::declared::{build_declared_public_api, detect_declared_dead_code};
pub use surface::exports::{ExportForm, ExportGraph, Facts, FileFacts, read_all, read_facts};
pub use surface::public_api::{
    PublicApiSurface, PublicApiSymbol, Stability, build_public_api, loaded_by_convention,
};
