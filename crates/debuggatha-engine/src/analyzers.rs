//! External analyzers: separate processes with a trust class, whose findings are merged with the
//! built-in detectors'. The deterministic core never depends on them.

mod adapters;
mod merge;
mod paths;
mod runner;
mod sarif;
mod types;

pub use adapters::{
    ADAPTERS, BIOME, CLIPPY, DETEKT, ESLINT, GITLEAKS, KTLINT, OSV_SCANNER, PHPSTAN, RUFF,
};
pub use merge::{analyzer_ids, merge_analyzer_findings};
pub use paths::to_repository_path;
pub use runner::{
    AnalyzerFindings, AnalyzerRunResult, InventoryEntry, RunInput, find_executable, inventory,
    path_dirs, run_analyzers,
};
pub use sarif::{SarifOptions, json_from, parse_sarif, severity_from_level};
pub use types::{
    Adapter, AnalyzerFinding, AnalyzerOptions, AnalyzerRun, Invocation, Mode, RepoFacts,
    RunContext, Status, Trust,
};
