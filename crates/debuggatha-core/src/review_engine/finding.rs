use std::collections::BTreeSet;
use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::env::Env;

/// Open on purpose: packs are data and may add categories (see `docs/schemas/README.md`).
pub type Category = String;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Confidence {
    High,
    Medium,
    Low,
}

// Declaration order is the order of seriousness; the derived `Ord` keeps maps in that order.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Critical,
    High,
    Medium,
    Low,
    Informational,
}

impl Severity {
    pub const ALL: [Severity; 5] = [
        Severity::Critical,
        Severity::High,
        Severity::Medium,
        Severity::Low,
        Severity::Informational,
    ];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct LineRange {
    #[schemars(range(min = 1))]
    pub start: u32,
    #[schemars(range(min = 1))]
    pub end: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct FindingLocation {
    pub file: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lines: Option<LineRange>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub columns: Option<LineRange>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum RecommendationAction {
    Replace,
    Remove,
    Add,
    Refactor,
    Configure,
    Document,
    Investigate,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RecommendationExample {
    pub before: String,
    pub after: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum EditSafety {
    Safe,
    Review,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RecommendationEdit {
    #[schemars(range(min = 1))]
    pub line: u32,
    #[schemars(range(min = 1))]
    pub start_column: u32,
    #[schemars(range(min = 1))]
    pub end_column: u32,
    pub replacement: String,
    pub safety: EditSafety,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Recommendation {
    pub id: String,
    pub action: RecommendationAction,
    pub summary: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rationale: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_file: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_lines: Option<LineRange>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub example: Option<RecommendationExample>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edit: Option<RecommendationEdit>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum SourceKind {
    AiProvider,
    DeterministicAnalyzer,
    CustomSkill,
    HumanReview,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum SemanticRole {
    Verification,
    Detection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum SemanticVerdict {
    Confirmed,
    Doubtful,
    Unsure,
}

/// Machine-readable evidence: a discriminated union, never free text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase"
)]
pub enum Evidence {
    Documentation {
        file: String,
        excerpt: String,
    },
    Criteria {
        rule_id: String,
        source: String,
    },
    ReviewPackRule {
        pack_id: String,
        rule_id: String,
    },
    FrameworkConvention {
        framework: String,
        detail: String,
    },
    LanguageConvention {
        language: String,
        detail: String,
    },
    Code {
        file: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        lines: Option<LineRange>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        excerpt: Option<String>,
    },
    ExternalAnalyzer {
        tool: String,
        rule_id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        version: Option<String>,
        license: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        url: Option<String>,
    },
    SemanticReview {
        role: SemanticRole,
        provider: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        model: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        verdict: Option<SemanticVerdict>,
        reason: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        topic: Option<String>,
    },
}

/// The rule a finding cites, as the ledger keys on it. First evidence that names one wins.
#[must_use]
pub fn rule_id_from_evidence(evidence: &[Evidence]) -> Option<String> {
    for item in evidence {
        match item {
            Evidence::ReviewPackRule { rule_id, .. } | Evidence::Criteria { rule_id, .. } => {
                return Some(rule_id.clone());
            }
            Evidence::ExternalAnalyzer { tool, rule_id, .. } => {
                return Some(format!("{tool}:{rule_id}"));
            }
            Evidence::SemanticReview {
                role: SemanticRole::Detection,
                topic: Some(topic),
                ..
            } if !topic.is_empty() => {
                return Some(format!("semantic:{topic}"));
            }
            _ => {}
        }
    }
    None
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PackReference {
    pub id: String,
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PolicyReference {
    pub id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Finding {
    pub id: String,
    pub title: String,
    pub explanation: String,
    pub severity: Severity,
    pub confidence: Confidence,
    pub category: Category,
    #[schemars(length(min = 1))]
    pub locations: Vec<FindingLocation>,
    #[schemars(length(min = 1))]
    pub evidence: Vec<Evidence>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub applied_policy: Option<PolicyReference>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub originating_pack: Option<PackReference>,
    pub recommendations: Vec<Recommendation>,
}

/// Distinct files a finding touches, in first-seen order.
#[must_use]
pub fn affected_files(finding: &Finding) -> Vec<String> {
    let mut seen = BTreeSet::new();
    finding
        .locations
        .iter()
        .filter(|l| seen.insert(l.file.as_str()))
        .map(|l| l.file.clone())
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FindingError {
    NoLocations(String),
    NoEvidence(String),
}

impl fmt::Display for FindingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoLocations(title) => write!(
                f,
                "Finding \"{title}\" has no locations — a finding must point at something."
            ),
            Self::NoEvidence(title) => write!(
                f,
                "Finding \"{title}\" has no evidence. Debuggatha never opines without evidence — see CLAUDE.md."
            ),
        }
    }
}

impl std::error::Error for FindingError {}

pub struct CreateFindingInput {
    pub id: Option<String>,
    pub title: String,
    pub explanation: String,
    pub severity: Severity,
    pub confidence: Confidence,
    pub category: Category,
    pub locations: Vec<FindingLocation>,
    pub evidence: Vec<Evidence>,
    pub applied_policy: Option<PolicyReference>,
    pub originating_pack: Option<PackReference>,
    pub recommendations: Vec<Recommendation>,
}

/// # Errors
/// A finding with no location or no evidence is refused: no source, no finding.
pub fn create_finding(input: CreateFindingInput, env: &dyn Env) -> Result<Finding, FindingError> {
    if input.locations.is_empty() {
        return Err(FindingError::NoLocations(input.title));
    }
    if input.evidence.is_empty() {
        return Err(FindingError::NoEvidence(input.title));
    }
    Ok(Finding {
        id: input.id.unwrap_or_else(|| env.new_id()),
        title: input.title,
        explanation: input.explanation,
        severity: input.severity,
        confidence: input.confidence,
        category: input.category,
        locations: input.locations,
        evidence: input.evidence,
        applied_policy: input.applied_policy,
        originating_pack: input.originating_pack,
        recommendations: input.recommendations,
    })
}
