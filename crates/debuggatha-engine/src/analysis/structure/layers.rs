//! The layer model and its violations. A boundary is placed in a conventional layer from the
//! words of its own name, never from what it depends on (that would be circular with the
//! violations, which consume the model). A name that fits no layer, or fits two, stays `unknown`
//! and is never used to accuse another module: the analysis does not assume one architecture.
//!
//! Differs from the TypeScript engine on purpose (`docs/PARITY.md`): it matched by suffix and by
//! `/word` prefix, so `ClientApp` was an application layer and `Webhooks.API` a presentation one.
//! Here a layer word must be a whole word of the name.

use std::collections::BTreeMap;

use serde::Serialize;

use super::boundaries::ModuleBoundary;
use super::graph::{DependencyGraph, Evidence};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LayerName {
    Domain,
    Application,
    Adapter,
    Infrastructure,
    Presentation,
    Unknown,
}

/// Innermost first: a layer may depend on the ones before it, never on the ones after.
pub const LAYER_ORDER: [LayerName; 5] = [
    LayerName::Domain,
    LayerName::Application,
    LayerName::Adapter,
    LayerName::Infrastructure,
    LayerName::Presentation,
];

impl LayerName {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Domain => "domain",
            Self::Application => "application",
            Self::Adapter => "adapter",
            Self::Infrastructure => "infrastructure",
            Self::Presentation => "presentation",
            Self::Unknown => "unknown",
        }
    }

    fn words(self) -> &'static [&'static str] {
        match self {
            Self::Domain => &["domain", "domains", "core", "entities", "model", "models"],
            Self::Application => &["application", "app", "use-cases", "usecases", "services"],
            Self::Adapter => &["adapter", "adapters", "gateways", "repositories"],
            Self::Infrastructure => &["infrastructure", "infra", "persistence", "data"],
            Self::Presentation => &["presentation", "ui", "web", "views", "components", "pages"],
            Self::Unknown => &[],
        }
    }
}

/// The layer a name belongs to: the one layer whose vocabulary has a word of the name.
fn match_layer(text: &str) -> LayerName {
    let lower = text.to_lowercase();
    // `use-cases` is one word of the vocabulary but two pieces of a name: try the name whole too.
    let words: Vec<&str> = lower
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .collect();
    let mut found = LAYER_ORDER.iter().copied().filter(|layer| {
        layer
            .words()
            .iter()
            .any(|vocab| words.contains(vocab) || (vocab.contains('-') && lower.contains(vocab)))
    });
    match (found.next(), found.next()) {
        (Some(layer), None) => layer,
        _ => LayerName::Unknown,
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct LayerModel {
    pub assignments: BTreeMap<String, LayerName>,
}

/// Places every boundary in a layer from its package name, or its directory when it has none.
#[must_use]
pub fn build_layer_model(boundaries: &[ModuleBoundary]) -> LayerModel {
    LayerModel {
        assignments: boundaries
            .iter()
            .map(|b| {
                (
                    b.id.clone(),
                    match_layer(b.name.as_deref().unwrap_or(&b.id)),
                )
            })
            .collect(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LayerViolation {
    pub from: String,
    pub to: String,
    pub from_layer: LayerName,
    pub to_layer: LayerName,
    /// Both sides of the violation, in words.
    pub explanation: String,
    pub evidence: Vec<Evidence>,
}

fn index(layer: LayerName) -> Option<usize> {
    LAYER_ORDER.iter().position(|l| *l == layer)
}

/// Edges that go from an inner layer to an outer one. Only an edge whose two ends are in known,
/// different layers is checked. One violation per pair even when both a declared and an import
/// edge exist: same problem, evidence merged.
#[must_use]
pub fn detect_layer_violations(graph: &DependencyGraph, model: &LayerModel) -> Vec<LayerViolation> {
    let mut by_pair: BTreeMap<(String, String), LayerViolation> = BTreeMap::new();
    for edge in &graph.edges {
        let layer = |id: &str| {
            model
                .assignments
                .get(id)
                .copied()
                .unwrap_or(LayerName::Unknown)
        };
        let (from_layer, to_layer) = (layer(&edge.from), layer(&edge.to));
        let (Some(from_index), Some(to_index)) = (index(from_layer), index(to_layer)) else {
            continue;
        };
        if from_index >= to_index {
            continue;
        }
        by_pair
            .entry((edge.from.clone(), edge.to.clone()))
            .or_insert_with(|| LayerViolation {
                from: edge.from.clone(),
                to: edge.to.clone(),
                from_layer,
                to_layer,
                explanation: format!(
                    "\"{}\" (layer \"{}\") depends on \"{}\" (layer \"{}\"), but \"{}\" is conventionally outer relative to \"{}\" — this is an upward dependency against the assumed layer order ({}).",
                    edge.from,
                    from_layer.as_str(),
                    edge.to,
                    to_layer.as_str(),
                    to_layer.as_str(),
                    from_layer.as_str(),
                    LAYER_ORDER.map(LayerName::as_str).join(" -> "),
                ),
                evidence: Vec::new(),
            })
            .evidence
            .extend(edge.evidence.iter().cloned());
    }
    by_pair.into_values().collect()
}
