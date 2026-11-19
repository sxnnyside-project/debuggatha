//! Splits findings by what active memory accepts; suppressed findings are reported, never hidden.

use serde::Serialize;

use super::api::MemoryStore;
use super::item::{MemoryDetail, MemoryItem};
use super::lifecycle::is_active_memory_status;
use super::matching::{accepted_deviation_matches, exception_matches, suppression_matches};
use crate::review_engine::Finding;

/// Why a finding was suppressed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SuppressionReason {
    /// The finding was accepted by fingerprint.
    Suppression,
    /// The rule was accepted for this scope.
    AcceptedDeviation,
    /// A deliberate exception covers it.
    Exception,
}

/// A finding an active memory item accepts; it is reported, never hidden.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SuppressedFinding {
    /// The finding.
    pub finding: Finding,
    /// The item that accepts it.
    pub memory_item_id: String,
    /// Why.
    pub reason: SuppressionReason,
}

/// The findings split by what active memory says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilterFindingsResult {
    /// The findings that remain.
    pub findings: Vec<Finding>,
    /// The findings memory suppressed.
    pub suppressed: Vec<SuppressedFinding>,
}

/// Move out the findings that an `active` memory item covers. They are reported, never hidden.
/// Order of precedence: suppression, accepted deviation, exception; within a kind, store order.
#[must_use]
pub fn filter_suppressed_findings(
    findings: &[Finding],
    store: &MemoryStore,
) -> FilterFindingsResult {
    let active = |pick: fn(&MemoryDetail) -> bool| -> Vec<&MemoryItem> {
        store
            .items
            .iter()
            .filter(|i| pick(&i.detail) && is_active_memory_status(i.status))
            .collect()
    };
    let suppressions = active(|d| matches!(d, MemoryDetail::Suppression { .. }));
    let deviations = active(|d| matches!(d, MemoryDetail::AcceptedDeviation { .. }));
    let exceptions = active(|d| matches!(d, MemoryDetail::Exception { .. }));

    let mut kept = Vec::new();
    let mut suppressed = Vec::new();
    for finding in findings {
        let hit = suppressions
            .iter()
            .find(|i| suppression_matches(i, finding))
            .map(|i| (i, SuppressionReason::Suppression))
            .or_else(|| {
                deviations
                    .iter()
                    .find(|i| accepted_deviation_matches(i, finding))
                    .map(|i| (i, SuppressionReason::AcceptedDeviation))
            })
            .or_else(|| {
                exceptions
                    .iter()
                    .find(|i| exception_matches(i, finding))
                    .map(|i| (i, SuppressionReason::Exception))
            });
        match hit {
            Some((item, reason)) => suppressed.push(SuppressedFinding {
                finding: finding.clone(),
                memory_item_id: item.id.clone(),
                reason,
            }),
            None => kept.push(finding.clone()),
        }
    }
    FilterFindingsResult {
        findings: kept,
        suppressed,
    }
}
