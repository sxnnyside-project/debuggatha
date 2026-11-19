use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::finding::{
    Finding, PackReference, PolicyReference, Recommendation, Severity, SourceKind,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ReviewSource {
    pub kind: SourceKind,
    pub name: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionMetadata {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<ReviewSource>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReviewSummary {
    pub total_findings: u32,
    pub findings_by_severity: BTreeMap<Severity, u32>,
    pub findings_by_category: BTreeMap<String, u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    pub applied_packs: Vec<PackReference>,
    pub applied_policies: Vec<PolicyReference>,
}

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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReviewResult {
    pub session_id: String,
    pub summary: ReviewSummary,
    pub findings: Vec<Finding>,
    pub recommendations: Vec<Recommendation>,
    pub execution: ExecutionMetadata,
}
