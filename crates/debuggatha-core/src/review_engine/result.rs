//! The result of a review and its summary.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::finding::{
    Finding, PackReference, PolicyReference, Recommendation, Severity, SourceKind,
};

/// What ran a review.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ReviewSource {
    /// The kind of source.
    pub kind: SourceKind,
    /// Its name.
    pub name: String,
}

/// Timing and outcome of a review.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionMetadata {
    /// What ran it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<ReviewSource>,
    /// When it started, ISO 8601.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started_at: Option<String>,
    /// When it ended, ISO 8601.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<String>,
    /// How long it took, in milliseconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    /// The failure, when the review failed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Counts that describe a review's findings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReviewSummary {
    /// How many findings there are.
    pub total_findings: u32,
    /// By severity; every severity is present, with 0 when none.
    pub findings_by_severity: BTreeMap<Severity, u32>,
    /// By category.
    pub findings_by_category: BTreeMap<String, u32>,
    /// How long the review took, in milliseconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    /// The packs applied.
    pub applied_packs: Vec<PackReference>,
    /// The policies applied.
    pub applied_policies: Vec<PolicyReference>,
}

/// The summary of `findings`, with every severity counted, including the ones with none.
#[must_use]
pub fn summarize_findings(
    findings: &[Finding],
    duration_ms: Option<u64>,
    applied_packs: Vec<PackReference>,
    applied_policies: Vec<PolicyReference>,
) -> ReviewSummary {
    let mut by_severity: BTreeMap<Severity, u32> = Severity::ALL.iter().map(|s| (*s, 0)).collect();
    let mut by_category: BTreeMap<String, u32> = BTreeMap::new();
    for finding in findings {
        *by_severity.entry(finding.severity).or_insert(0) += 1;
        *by_category.entry(finding.category.clone()).or_insert(0) += 1;
    }
    ReviewSummary {
        total_findings: u32::try_from(findings.len()).unwrap_or(u32::MAX),
        findings_by_severity: by_severity,
        findings_by_category: by_category,
        duration_ms,
        applied_packs,
        applied_policies,
    }
}

/// The result of a review.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReviewResult {
    /// The session that produced it.
    pub session_id: String,
    /// The counts.
    pub summary: ReviewSummary,
    /// The findings, most serious first.
    pub findings: Vec<Finding>,
    /// What to do, across findings.
    pub recommendations: Vec<Recommendation>,
    /// Timing and outcome.
    pub execution: ExecutionMetadata,
}
