//! The types of a repository context: stack, capabilities, dependencies, documentation and criteria.

use indexmap::IndexMap;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A fact and where it comes from. Every claim Debuggatha makes about a repository carries one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Evidence {
    /// The file the fact comes from, relative to the repository root.
    pub file: String,
    /// What the file says, in a sentence.
    pub detail: String,
}

impl Evidence {
    /// Evidence from `file`, with what it says.
    #[must_use]
    pub fn new(file: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            file: file.into(),
            detail: detail.into(),
        }
    }
}

/// A detected value (a language, a framework...) and the evidence for it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct StackSignal {
    /// The value, such as `rust` or `react`.
    pub value: String,
    /// Where it was seen; never empty.
    pub evidence: Vec<Evidence>,
}

/// What kind of product the repository builds.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "lowercase")]
pub enum PlatformTarget {
    /// A desktop application.
    Desktop,
    /// A mobile application.
    Mobile,
    /// A website or web application.
    Web,
}

impl PlatformTarget {
    /// The name used in reports and rule scopes.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Desktop => "desktop",
            Self::Mobile => "mobile",
            Self::Web => "web",
        }
    }
}

/// How the repository is laid out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum WorkspaceType {
    /// One package or project at the root.
    SinglePackage,
    /// Several packages under one root (a workspace).
    Monorepo,
}

/// What Stack Detection read from the manifests: the languages, frameworks and tools in use.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct StackProfile {
    /// Languages in use.
    pub languages: Vec<StackSignal>,
    /// Frameworks in use.
    pub frameworks: Vec<StackSignal>,
    /// Build systems (Cargo, Gradle, Vite...).
    pub build_systems: Vec<StackSignal>,
    /// Package managers (npm, pnpm, Composer...).
    pub package_managers: Vec<StackSignal>,
    /// Runtimes the code targets.
    pub runtimes: Vec<StackSignal>,
    /// The kinds of product the repository builds.
    pub platform_targets: Vec<PlatformTarget>,
    /// Single package or monorepo.
    pub workspace_type: WorkspaceType,
    /// Why the workspace type was chosen.
    pub workspace_evidence: Vec<Evidence>,
    /// The manifests that were read, relative to the root.
    pub manifests: Vec<String>,
}

/// The group a [`Capability`] belongs to.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "lowercase")]
pub enum CapabilityKind {
    /// A programming language.
    Language,
    /// A framework or library family.
    Framework,
    /// A platform it targets.
    Platform,
    /// A tool in the build or quality chain.
    Tooling,
    /// A property of the project, such as being a monorepo.
    Characteristic,
}

/// How sure a detection is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Confidence {
    /// Read directly from a manifest or lockfile.
    High,
    /// Inferred from content or several signals.
    Medium,
    /// A weak signal; stated so, never presented as fact.
    Low,
}

/// Where the evidence for a capability came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum CapabilityOrigin {
    /// A manifest such as `package.json` or `Cargo.toml`.
    Manifest,
    /// A lockfile.
    Lockfile,
    /// A configuration file.
    Config,
    /// A convention (a file or folder name).
    Convention,
}

/// What the repository is, in normalized terms packs can match on (`typescript`, `react`, ...).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Capability {
    /// Normalized identifier packs match on, such as `typescript`.
    pub id: String,
    /// Which group it belongs to.
    pub kind: CapabilityKind,
    /// How sure the detection is.
    pub confidence: Confidence,
    /// What it was detected from; never empty.
    pub evidence: Vec<Evidence>,
    /// The kind of source it came from.
    pub origin: CapabilityOrigin,
}

/// What the repository depends on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DependencyProfile {
    /// Lockfiles found.
    pub lockfiles: Vec<Evidence>,
    /// Dependency versions as declared, by name.
    pub declared_versions: IndexMap<String, String>,
    /// Constraints on the runtime (an engines or rust-version field), by name.
    pub runtime_constraints: IndexMap<String, String>,
}

/// The kind of a documentation file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum DocumentationKind {
    /// A README.
    Readme,
    /// A CLAUDE.md or similar agent memory file.
    ClaudeMemory,
    /// A contributing guide.
    Contributing,
    /// An architecture document.
    Architecture,
    /// An architecture decision record.
    Adr,
    /// A product requirements document.
    Prd,
    /// A roadmap.
    Roadmap,
    /// Any other documentation.
    Other,
}

impl DocumentationKind {
    /// The name used in reports.
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

/// One documentation file that was read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DocumentationSource {
    /// Path relative to the repository root.
    pub file: String,
    /// What kind of document it is.
    pub kind: DocumentationKind,
    /// Its first heading, when it has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Length in words.
    pub word_count: u64,
}

/// What the repository's documentation says about its intent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DocumentationProfile {
    /// The files that were read.
    pub sources: Vec<DocumentationSource>,
    /// A short summary of the intent they state.
    pub summary: String,
}

/// A criterion the repository declares for its own code.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CriteriaRule {
    /// Stable identifier.
    pub id: String,
    /// The criterion in a sentence.
    pub description: String,
    /// The file or document that states it.
    pub source: Evidence,
}

/// The repository's own criteria, resolved from documentation, configuration and manifests.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CriteriaProfile {
    /// The criteria found.
    pub rules: Vec<CriteriaRule>,
    /// The files they came from.
    pub sources: Vec<String>,
}

/// A question Debuggatha raises because a concrete gap exists; it never invents an answer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct OpenQuestion {
    /// What is unknown.
    pub question: String,
    /// Why it matters to a review.
    pub reason: String,
}

/// How well Debuggatha understands the repository, and what it still does not.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct UnderstandingProfile {
    /// Overall confidence in the understanding.
    pub confidence: Confidence,
    /// The gaps that remain.
    pub open_questions: Vec<OpenQuestion>,
}

/// One normalized, immutable description of a repository. Everything downstream consumes this
/// instead of touching the file system.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryContext {
    /// Absolute path of the repository root.
    pub root_dir: String,
    /// When the context was built, as an ISO 8601 timestamp.
    #[schemars(extend("format" = "date-time"))]
    pub generated_at: String,
    /// Whether the root is a git repository.
    pub has_git: bool,
    /// The stack: languages, frameworks, tools.
    pub stack: StackProfile,
    /// The capabilities rules match against.
    pub capabilities: Vec<Capability>,
    /// What the repository depends on.
    pub dependencies: DependencyProfile,
    /// What its documentation says.
    pub documentation: DocumentationProfile,
    /// Its own criteria.
    pub criteria: CriteriaProfile,
    /// What Debuggatha understands and what it does not.
    pub understanding: UnderstandingProfile,
}

/// A directory as it was listed when the context was built, for the cache fingerprint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirListing {
    /// The directory that was listed.
    pub dir: std::path::PathBuf,
    /// The names it held.
    pub entries: Vec<String>,
}

/// What a scanner returns: its profile, and what it read (so a cache knows when to rebuild).
#[derive(Debug, Clone)]
pub struct ScanResult<T> {
    /// What the scanner found.
    pub profile: T,
    /// The files it read, so a cache knows when to rebuild.
    pub files_read: Vec<std::path::PathBuf>,
    /// The directories it listed.
    pub dirs_listed: Vec<DirListing>,
}

/// Why a scan failed.
#[derive(Debug)]
pub enum ScanError {
    /// A file or directory could not be read.
    Io {
        /// What could not be read.
        path: std::path::PathBuf,
        /// The operating system's error.
        source: std::io::Error,
    },
    /// A manifest that must parse, and does not. The text is the parser's, not V8's.
    InvalidJson {
        /// The manifest, relative to the root.
        file: String,
        /// What the parser said.
        why: String,
    },
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
