//! Every stage over every language, in one call, with what could not be read said aloud.
//!
//! Each module gets the reader its ecosystem calls for: the entrypoints of a JavaScript package, the
//! `pub` reachability of a Rust crate, and the declarations of any other language the semantic model
//! reads. A language with no reader is listed in [`Analysis::not_covered`] and never reported clean.

use std::collections::BTreeMap;
use std::path::Path;

use super::change_impact::{ChangeImpact, estimate_change_impact, estimate_change_impact_of_diff};
use super::ecosystem::Ecosystems;
use super::ownership::{OwnershipSignal, detect_ownership};
use super::structure::boundaries::{Boundaries, infer_boundaries};
use super::structure::cycles::{DependencyCycle, detect_cycles};
use super::structure::graph::{DependencyGraph, build_graph};
use super::structure::layers::{
    LayerModel, LayerViolation, build_layer_model, detect_layer_violations,
};
use super::structure::resolve::Resolver;
use super::structure::workspace::Workspace;
use super::surface::dead_code::{DeadCodeFinding, detect_dead_code};
use super::surface::declared::{build_declared_public_api, detect_declared_dead_code};
use super::surface::exports::{ExportGraph, read_all};
use super::surface::public_api::{PublicApiSurface, build_public_api};
use crate::diff::FileDiff;
use crate::syntax::{Backend, Language};

/// Files of a language no stage reads, and what is missing for them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotCovered {
    pub language: &'static str,
    pub files: usize,
    pub what: &'static str,
}

#[derive(Debug)]
pub struct Analysis {
    pub boundaries: Boundaries,
    pub graph: DependencyGraph,
    pub layers: LayerModel,
    pub layer_violations: Vec<LayerViolation>,
    pub cycles: Vec<DependencyCycle>,
    pub ownership: Vec<OwnershipSignal>,
    pub public_api: Vec<PublicApiSurface>,
    pub dead_code: Vec<DeadCodeFinding>,
    pub not_covered: Vec<NotCovered>,
}

impl Analysis {
    /// Which modules a set of changed files reaches, and which depend on them.
    #[must_use]
    pub fn change_impact(&self, changed: &[String]) -> ChangeImpact {
        estimate_change_impact(
            changed,
            &self.boundaries.list,
            &self.graph,
            &self.public_api,
        )
    }

    /// The same estimate from a parsed diff, down to the export statements it touches.
    #[must_use]
    pub fn change_impact_of_diff(&self, diff: &[FileDiff]) -> ChangeImpact {
        estimate_change_impact_of_diff(diff, &self.boundaries.list, &self.graph, &self.public_api)
    }
}

/// Runs every stage over the workspace.
#[must_use]
pub fn analyze(ws: &Workspace) -> Analysis {
    let boundaries = infer_boundaries(ws);
    let graph = build_graph(ws, &boundaries);
    let layers = build_layer_model(&boundaries.list);
    let layer_violations = detect_layer_violations(&graph, &layers);
    let cycles = detect_cycles(&graph);
    let ownership = detect_ownership(ws, &boundaries.list);

    let facts = read_all(ws);
    let resolver = Resolver::new(ws, &boundaries.list);
    let exports = ExportGraph {
        facts: &facts,
        resolver: &resolver,
    };
    let entrypoint_based = build_public_api(ws, &boundaries.list, &facts, &exports, &resolver);
    let declared = build_declared_public_api(ws, &boundaries.list);
    let public_api: Vec<PublicApiSurface> = entrypoint_based
        .into_iter()
        .map(|surface| {
            if surface.covered {
                return surface;
            }
            declared
                .iter()
                .find(|d| d.module_id == surface.module_id)
                .cloned()
                .unwrap_or(surface)
        })
        .collect();

    let mut dead_code = detect_dead_code(
        ws,
        &boundaries.list,
        &facts,
        &exports,
        &resolver,
        &public_api,
    );
    dead_code.extend(detect_declared_dead_code(ws));

    let not_covered = not_covered(ws);
    Analysis {
        boundaries,
        graph,
        layers,
        layer_violations,
        cycles,
        ownership,
        public_api,
        dead_code,
        not_covered,
    }
}

/// Languages whose files are in the workspace and have no module reader: not JavaScript or a
/// component, not Rust (read through the crate's module tree), and no ecosystem descriptor.
fn not_covered(ws: &Workspace) -> Vec<NotCovered> {
    let mut count: BTreeMap<&'static str, usize> = BTreeMap::new();
    for file in ws.owned_files() {
        let Some(language) = Language::from_path(Path::new(file)) else {
            continue;
        };
        let read = matches!(language.backend(), Backend::Oxc | Backend::Sfc)
            || language == Language::Rust
            || language == Language::Html
            || Ecosystems::of(language).is_some();
        if !read {
            *count.entry(language.id()).or_default() += 1;
        }
    }
    let mut out: Vec<NotCovered> = count
        .into_iter()
        .map(|(language, files)| NotCovered {
            language,
            files,
            what: "no module boundaries, imports, public API or dead code are read",
        })
        .collect();
    let rust = ws
        .owned_files()
        .filter(|f| Language::from_path(Path::new(f)) == Some(Language::Rust))
        .count();
    if rust > 0 {
        out.push(NotCovered {
            language: "rust",
            files: rust,
            what: "dead code is the compiler's: the `dead_code` and `unreachable_pub` lints",
        });
    }
    out
}
