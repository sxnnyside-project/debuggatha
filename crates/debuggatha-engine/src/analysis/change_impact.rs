//! Change impact: which modules a set of changed files belongs to, which modules depend on them
//! (through the graph, transitively), and which names of a module's public surface the change
//! touches. It states how much of the estimate it could see: a changed file no module owns, a file
//! the graph could not read, and imports that resolve to nothing all lower the confidence, and the
//! rationale says which.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use serde::Serialize;

use crate::diff::{ChangeKind, FileDiff};

use super::boundaries::ModuleBoundary;
use super::graph::DependencyGraph;
use super::public_api::{PublicApiSurface, PublicApiSymbol};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ImpactConfidence {
    High,
    Medium,
    Low,
}

impl ImpactConfidence {
    fn min_with(self, ceiling: Self) -> Self {
        match (self, ceiling) {
            (Self::Low, _) | (_, Self::Low) => Self::Low,
            (Self::Medium, _) | (_, Self::Medium) => Self::Medium,
            _ => Self::High,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ChangeImpact {
    pub changed_files: Vec<String>,
    pub affected_modules: Vec<String>,
    /// Modules not changed that depend, transitively, on a changed one.
    pub downstream_modules: Vec<String>,
    /// Public names whose statement or declaration sits in a changed file.
    pub affected_public_api: Vec<PublicApiSymbol>,
    /// Changed files no module owns (a language the analysis does not read, a file outside any
    /// boundary).
    pub unmapped_files: Vec<String>,
    /// Of the affected public names, those whose export statement sits on a line the diff touched.
    /// Empty without a diff; the declaration itself is only known to be in a changed file.
    pub exports_on_changed_lines: Vec<PublicApiSymbol>,
    /// Files the diff removes: they are not in the graph, so what imported them is not followed.
    pub deleted_files: Vec<String>,
    pub confidence: ImpactConfidence,
    pub rationale: String,
}

/// Modules that depend, through the graph and transitively, on any of `start` (not `start`).
fn dependents_of<'g>(graph: &'g DependencyGraph, start: &BTreeSet<&'g str>) -> BTreeSet<&'g str> {
    let mut dependents: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for e in &graph.edges {
        dependents
            .entry(e.to.as_str())
            .or_default()
            .push(e.from.as_str());
    }
    let mut seen: BTreeSet<&str> = start.clone();
    let mut queue: VecDeque<&str> = start.iter().copied().collect();
    let mut downstream: BTreeSet<&str> = BTreeSet::new();
    while let Some(m) = queue.pop_front() {
        for d in dependents.get(m).into_iter().flatten() {
            if seen.insert(d) {
                downstream.insert(d);
                queue.push_back(d);
            }
        }
    }
    downstream
}

/// How far the estimate can be trusted, and why.
fn assess(
    graph: &DependencyGraph,
    changed: &[String],
    mapped: usize,
    unmapped: usize,
    public: usize,
) -> (ImpactConfidence, String) {
    if mapped == 0 {
        return (
            ImpactConfidence::Low,
            "None of the changed files belongs to a known module, so no dependent could be found."
                .to_owned(),
        );
    }
    let unread = changed
        .iter()
        .filter(|f| graph.unread_files.contains(f) || graph.unrooted_files.contains(f))
        .count();
    let computed = graph
        .unresolved
        .iter()
        .filter(|u| u.specifier.is_none())
        .count();
    let mut reasons = Vec::new();
    if unmapped > 0 {
        reasons.push(format!(
            "{unmapped} changed file(s) belong to no module and add nothing to the estimate"
        ));
    }
    if unread > 0 {
        reasons.push(format!(
            "{unread} changed file(s) could not be read into the graph, so what they import is missing"
        ));
    }
    if computed > 0 {
        reasons.push(format!(
            "{computed} import(s) in the repository name a computed path and are not in the graph"
        ));
    }
    if public > 0 {
        reasons.push(format!(
            "{public} changed name(s) are part of a public surface, whose consumers outside this repository cannot be measured"
        ));
    }
    if reasons.is_empty() {
        (
            ImpactConfidence::High,
            "Every changed file belongs to a module and the graph was read in full.".to_owned(),
        )
    } else {
        (ImpactConfidence::Medium, format!("{}.", reasons.join("; ")))
    }
}

#[must_use]
pub fn estimate_change_impact(
    changed: &[String],
    boundaries: &[ModuleBoundary],
    graph: &DependencyGraph,
    apis: &[PublicApiSurface],
) -> ChangeImpact {
    let owner: BTreeMap<&str, &str> = boundaries
        .iter()
        .flat_map(|b| b.files.iter().map(move |f| (f.as_str(), b.id.as_str())))
        .collect();
    let mut changed_files: Vec<String> = changed.to_vec();
    changed_files.sort();
    changed_files.dedup();

    let mut affected: BTreeSet<&str> = BTreeSet::new();
    let mut unmapped = Vec::new();
    for f in &changed_files {
        match owner.get(f.as_str()) {
            Some(id) => {
                affected.insert(id);
            }
            None => unmapped.push(f.clone()),
        }
    }
    let downstream = dependents_of(graph, &affected);
    let public: Vec<PublicApiSymbol> = apis
        .iter()
        .filter(|a| affected.contains(a.module_id.as_str()))
        .flat_map(|a| a.exported.iter())
        .filter(|s| {
            changed_files.contains(&s.file)
                || s.definition
                    .as_ref()
                    .is_some_and(|(f, _)| changed_files.contains(f))
        })
        .cloned()
        .collect();
    let (confidence, rationale) = assess(
        graph,
        &changed_files,
        affected.len(),
        unmapped.len(),
        public.len(),
    );
    ChangeImpact {
        changed_files,
        affected_modules: affected.iter().map(|s| (*s).to_owned()).collect(),
        downstream_modules: downstream.iter().map(|s| (*s).to_owned()).collect(),
        affected_public_api: public,
        unmapped_files: unmapped,
        exports_on_changed_lines: Vec::new(),
        deleted_files: Vec::new(),
        confidence,
        rationale,
    }
}

/// The estimate for a diff: the files it changes, plus which exports it touches line by line. A
/// removed file cannot be followed through the graph, which is read from the tree as it is now, so
/// the estimate says so and is never `High`.
#[must_use]
pub fn estimate_change_impact_of_diff(
    diff: &[FileDiff],
    boundaries: &[ModuleBoundary],
    graph: &DependencyGraph,
    apis: &[PublicApiSurface],
) -> ChangeImpact {
    let paths: Vec<String> = diff
        .iter()
        .filter(|f| !f.binary)
        .map(|f| f.path.clone())
        .collect();
    let mut impact = estimate_change_impact(&paths, boundaries, graph, apis);
    impact.exports_on_changed_lines = impact
        .affected_public_api
        .iter()
        .filter(|s| {
            diff.iter()
                .any(|f| f.path == s.file && f.touched_lines().contains(&s.line))
        })
        .cloned()
        .collect();
    impact.deleted_files = diff
        .iter()
        .filter(|f| f.kind == ChangeKind::Deleted)
        .map(|f| f.path.clone())
        .collect();
    if !impact.deleted_files.is_empty() {
        impact.confidence = impact.confidence.min_with(ImpactConfidence::Medium);
        impact.rationale = format!(
            "{} removed file(s) are not in the graph, so the modules that imported them are not followed; {}",
            impact.deleted_files.len(),
            impact.rationale
        );
    }
    impact
}
