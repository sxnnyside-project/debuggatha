//! The finding: severity, confidence, locations, evidence and recommendations. A finding cannot be built without evidence and a location.

use std::collections::BTreeSet;
use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::env::Env;

/// Open on purpose: packs are data and may add categories (see `docs/schemas/README.md`).
pub type Category = String;

/// How sure a detector is that a finding is a real problem, independent of its [`Severity`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Confidence {
    /// The syntax and resolved facts make the claim certain.
    High,
    /// Probable; part of the evidence could not be resolved.
    Medium,
    /// A suspicion worth a look, with its limits stated in the rule.
    Low,
}

// Declaration order is the order of seriousness; the derived `Ord` keeps maps in that order.
/// How serious a finding is if it is real, independent of its [`Confidence`]. Declared from most to least serious, so the derived order is the order of seriousness.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    /// Exploitable or data-destroying; fix before shipping.
    Critical,
    /// A defect that will break users or leak data under ordinary use.
    High,
    /// A defect or risk that matters under some conditions.
    Medium,
    /// A small defect or a departure from convention.
    Low,
    /// Worth knowing; no action is required.
    Informational,
}

impl Severity {
    /// Every severity, most serious first.
    pub const ALL: [Severity; 5] = [
        Severity::Critical,
        Severity::High,
        Severity::Medium,
        Severity::Low,
        Severity::Informational,
    ];
}

/// A span of lines or columns, 1-based and inclusive at both ends.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct LineRange {
    /// First line (or column) of the span.
    #[schemars(range(min = 1))]
    pub start: u32,
    /// Last line (or column) of the span, equal to `start` for a single one.
    #[schemars(range(min = 1))]
    pub end: u32,
}

/// Where a finding is: a file and, when known, the lines and columns.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct FindingLocation {
    /// Path relative to the repository root, with `/` separators.
    pub file: String,
    /// The lines the finding is about; `None` when it concerns the whole file.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lines: Option<LineRange>,
    /// The columns on the first line, when the finding names an exact span.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub columns: Option<LineRange>,
}

/// What a recommendation asks the reader to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum RecommendationAction {
    /// Swap one piece of code for another.
    Replace,
    /// Delete code.
    Remove,
    /// Add code that is missing.
    Add,
    /// Restructure without changing behavior.
    Refactor,
    /// Change a setting or a configuration file.
    Configure,
    /// Write or update documentation.
    Document,
    /// Look closer before deciding; the engine could not.
    Investigate,
}

/// The same code before and after a fix, shown to the reader.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RecommendationExample {
    /// The code as it is.
    pub before: String,
    /// The code as it should be.
    pub after: String,
}

/// How far an edit can be trusted to be applied as it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum EditSafety {
    /// Mechanical and behavior-preserving; a client may apply it as a preferred fix.
    Safe,
    /// Plausible, but the person must read the result first.
    Review,
}

/// An exact text replacement on one line, for fixes that are mechanical.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RecommendationEdit {
    /// The line the edit applies to.
    #[schemars(range(min = 1))]
    pub line: u32,
    /// First column replaced.
    #[schemars(range(min = 1))]
    pub start_column: u32,
    /// Column after the last one replaced.
    #[schemars(range(min = 1))]
    pub end_column: u32,
    /// The text that takes the place of the span.
    pub replacement: String,
    /// Whether the edit is safe to apply without reading it.
    pub safety: EditSafety,
}

/// What to do about a finding: a summary, why, and optionally an example or an exact edit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Recommendation {
    /// Identifier, unique within its finding.
    pub id: String,
    /// The kind of change asked for.
    pub action: RecommendationAction,
    /// One sentence saying what to do.
    pub summary: String,
    /// Why this is the right change.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rationale: Option<String>,
    /// The file the change belongs in, when not the one the finding is in.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_file: Option<String>,
    /// The lines the change touches.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_lines: Option<LineRange>,
    /// The code before and after, when the fix is not mechanical.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub example: Option<RecommendationExample>,
    /// An exact replacement, when the fix is mechanical.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edit: Option<RecommendationEdit>,
}

/// What produced a piece of evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum SourceKind {
    /// A model, through the optional semantic layer.
    AiProvider,
    /// A detector or an external analyzer.
    DeterministicAnalyzer,
    /// A skill supplied by the repository or a pack.
    CustomSkill,
    /// A person.
    HumanReview,
}

/// What the semantic layer was asked to do with a finding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum SemanticRole {
    /// Judge a finding a detector raised.
    Verification,
    /// Read code for defects detectors do not see.
    Detection,
}

/// A model's judgment of a finding a detector raised; it never closes, hides or lowers the finding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum SemanticVerdict {
    /// The model agrees it is a real problem.
    Confirmed,
    /// The model doubts it, quoting the line that makes it look safe.
    Doubtful,
    /// The model could not tell.
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
    /// A passage of the repository's own documentation.
    Documentation {
        /// The documentation file.
        file: String,
        /// The passage quoted.
        excerpt: String,
    },
    /// A criterion the repository states or implies.
    Criteria {
        /// Identifier of the criterion.
        rule_id: String,
        /// Where the repository states it (a config file or a document).
        source: String,
    },
    /// A rule of a Review Pack.
    ReviewPackRule {
        /// The pack the rule belongs to.
        pack_id: String,
        /// The rule's identifier within the pack.
        rule_id: String,
    },
    /// A convention of a framework.
    FrameworkConvention {
        /// The framework's name.
        framework: String,
        /// The convention, in a sentence.
        detail: String,
    },
    /// A convention of a language.
    LanguageConvention {
        /// The language's name.
        language: String,
        /// The convention, in a sentence.
        detail: String,
    },
    /// The analyzed code itself.
    Code {
        /// The file the code is in.
        file: String,
        /// The lines quoted.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        lines: Option<LineRange>,
        /// The code, redacted of anything that looks like a secret.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        excerpt: Option<String>,
    },
    /// The output of an external analyzer, which runs as a separate process.
    ExternalAnalyzer {
        /// The tool's name.
        tool: String,
        /// The tool's own rule identifier.
        rule_id: String,
        /// The tool's version, when it reports one.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        version: Option<String>,
        /// The tool's license, from the analyzer catalog.
        license: String,
        /// A page that explains the rule.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        url: Option<String>,
    },
    /// What the optional semantic layer concluded.
    SemanticReview {
        /// What the model was asked to do.
        role: SemanticRole,
        /// The provider that served the request (a local runtime or the client's own model).
        provider: String,
        /// The model's name, when known.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        model: Option<String>,
        /// The verdict, for a verification.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        verdict: Option<SemanticVerdict>,
        /// The reason the model gave, kept only when it quotes the line it is about.
        reason: String,
        /// The subject of a detection, when it has one.
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

/// A pack, by id and exact version, so a finding stays citable after the pack updates (ADR-0002).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PackReference {
    /// The pack's namespaced id, such as `debuggatha/rust`.
    pub id: String,
    /// The pack's semantic version.
    pub version: String,
}

/// A review policy, by its deterministic id (ADR-0005).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PolicyReference {
    /// The policy's id.
    pub id: String,
}

/// A problem found in the code, always with at least one location and one piece of evidence: no source, no finding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Finding {
    /// Stable identifier of the finding within its review.
    pub id: String,
    /// A short statement of the problem.
    pub title: String,
    /// Why it is a problem, in the reviewer's words.
    pub explanation: String,
    /// How serious it is if real.
    pub severity: Severity,
    /// How sure the detector is.
    pub confidence: Confidence,
    /// The area it belongs to (security, reliability, architecture...).
    pub category: Category,
    /// Where it is; never empty.
    #[schemars(length(min = 1))]
    pub locations: Vec<FindingLocation>,
    /// What backs it up; never empty.
    #[schemars(length(min = 1))]
    pub evidence: Vec<Evidence>,
    /// The policy the review applied, when the finding came from one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub applied_policy: Option<PolicyReference>,
    /// The pack whose rule produced it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub originating_pack: Option<PackReference>,
    /// What to do about it.
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

/// Why a finding could not be built.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FindingError {
    /// No location was given; the string is the finding's title.
    NoLocations(String),
    /// No evidence was given; the string is the finding's title.
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

/// What [`Finding`] needs to be built; the id is generated when absent.
pub struct CreateFindingInput {
    /// The id to use, or `None` to derive one.
    pub id: Option<String>,
    /// A short statement of the problem.
    pub title: String,
    /// Why it is a problem.
    pub explanation: String,
    /// How serious it is if real.
    pub severity: Severity,
    /// How sure the detector is.
    pub confidence: Confidence,
    /// The area it belongs to.
    pub category: Category,
    /// Where it is; must not be empty.
    pub locations: Vec<FindingLocation>,
    /// What backs it up; must not be empty.
    pub evidence: Vec<Evidence>,
    /// The policy the review applied.
    pub applied_policy: Option<PolicyReference>,
    /// The pack whose rule produced it.
    pub originating_pack: Option<PackReference>,
    /// What to do about it.
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
