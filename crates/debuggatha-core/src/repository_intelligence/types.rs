use indexmap::IndexMap;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A fact and where it comes from. Every claim Debuggatha makes about a repository carries one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Evidence {
    pub file: String,
    pub detail: String,
}

impl Evidence {
    #[must_use]
    pub fn new(file: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            file: file.into(),
            detail: detail.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct StackSignal {
    pub value: String,
    pub evidence: Vec<Evidence>,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "lowercase")]
pub enum PlatformTarget {
    Desktop,
    Mobile,
    Web,
}

impl PlatformTarget {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Desktop => "desktop",
            Self::Mobile => "mobile",
            Self::Web => "web",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum WorkspaceType {
    SinglePackage,
    Monorepo,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct StackProfile {
    pub languages: Vec<StackSignal>,
    pub frameworks: Vec<StackSignal>,
    pub build_systems: Vec<StackSignal>,
    pub package_managers: Vec<StackSignal>,
    pub runtimes: Vec<StackSignal>,
    pub platform_targets: Vec<PlatformTarget>,
    pub workspace_type: WorkspaceType,
    pub workspace_evidence: Vec<Evidence>,
    pub manifests: Vec<String>,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "lowercase")]
pub enum CapabilityKind {
    Language,
    Framework,
    Platform,
    Tooling,
    Characteristic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Confidence {
    High,
    Medium,
    Low,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum CapabilityOrigin {
    Manifest,
    Lockfile,
    Config,
    Convention,
}

/// What the repository is, in normalized terms packs can match on (`typescript`, `react`, ...).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Capability {
    pub id: String,
    pub kind: CapabilityKind,
    pub confidence: Confidence,
    pub evidence: Vec<Evidence>,
    pub origin: CapabilityOrigin,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DependencyProfile {
    pub lockfiles: Vec<Evidence>,
    pub declared_versions: IndexMap<String, String>,
    pub runtime_constraints: IndexMap<String, String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum DocumentationKind {
    Readme,
    ClaudeMemory,
    Contributing,
    Architecture,
    Adr,
    Prd,
    Roadmap,
    Other,
}

impl DocumentationKind {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Readme => "readme",
            Self::ClaudeMemory => "claude-memory",
            Self::Contributing => "contributing",
            Self::Architecture => "architecture",
            Self::Adr => "adr",
            Self::Prd => "prd",
            Self::Roadmap => "roadmap",
            Self::Other => "other",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DocumentationSource {
    pub file: String,
    pub kind: DocumentationKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub word_count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DocumentationProfile {
    pub sources: Vec<DocumentationSource>,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CriteriaRule {
    pub id: String,
    pub description: String,
    pub source: Evidence,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CriteriaProfile {
    pub rules: Vec<CriteriaRule>,
    pub sources: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct OpenQuestion {
    pub question: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct UnderstandingProfile {
    pub confidence: Confidence,
    pub open_questions: Vec<OpenQuestion>,
}

/// One normalized, immutable description of a repository. Everything downstream consumes this
/// instead of touching the file system.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryContext {
    pub root_dir: String,
    #[schemars(extend("format" = "date-time"))]
    pub generated_at: String,
    pub has_git: bool,
    pub stack: StackProfile,
    pub capabilities: Vec<Capability>,
    pub dependencies: DependencyProfile,
    pub documentation: DocumentationProfile,
    pub criteria: CriteriaProfile,
    pub understanding: UnderstandingProfile,
}

/// A directory as it was listed when the context was built, for the cache fingerprint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirListing {
    pub dir: std::path::PathBuf,
    pub entries: Vec<String>,
}

/// What a scanner returns: its profile, and what it read (so a cache knows when to rebuild).
#[derive(Debug, Clone)]
pub struct ScanResult<T> {
    pub profile: T,
    pub files_read: Vec<std::path::PathBuf>,
    pub dirs_listed: Vec<DirListing>,
}

#[derive(Debug)]
pub enum ScanError {
    Io {
        path: std::path::PathBuf,
        source: std::io::Error,
    },
    /// A manifest that must parse, and does not. The text is the parser's, not V8's.
    InvalidJson { file: String, why: String },
}

impl std::fmt::Display for ScanError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io { path, source } => write!(f, "Cannot read {}: {source}", path.display()),
            Self::InvalidJson { file, why } => write!(f, "Cannot parse {file}: {why}"),
        }
    }
}

impl std::error::Error for ScanError {}
