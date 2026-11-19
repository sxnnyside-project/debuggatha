//! The shape of a Review Pack: rules, knowledge, dependencies and skills.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::review_engine::{Category, Severity};

/// Which repositories a rule applies to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum RuleScope {
    /// Every repository.
    Always,
    /// Repositories that use a framework.
    RequiresFramework {
        /// The framework's capability id.
        framework: String,
    },
    /// Repositories that use a language.
    RequiresLanguage {
        /// The language's capability id.
        language: String,
    },
    /// Files that match a glob.
    Glob {
        /// The glob, relative to the repository root.
        pattern: String,
    },
}

/// Domain expertise a rule derives from, kept next to the rules so they can cite it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgeEntry {
    /// Stable identifier, referenced by `Rule::knowledge_refs`.
    pub id: String,
    /// A short title.
    pub title: String,
    /// The explanation.
    pub body: String,
    /// Outside references (standards, articles).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub external_refs: Option<Vec<String>>,
    /// What the guidance does not cover or gets wrong.
    pub limitations: Vec<String>,
}

/// One atomic, individually citable statement a review can raise.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Rule {
    /// Identifier, unique within its pack.
    pub id: String,
    /// The pack it belongs to.
    pub pack_id: String,
    /// The rule, as a sentence a finding can quote.
    pub statement: String,
    /// The area it belongs to.
    pub category: Category,
    /// Where it applies.
    pub applies_to: RuleScope,
    /// Severity of a finding of this rule unless a detector says otherwise.
    pub default_severity: Severity,
    /// The knowledge entries it derives from.
    pub knowledge_refs: Vec<String>,
    /// Rules it contradicts; the conflict is recorded, never dropped (ADR-0003).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contradicts: Option<Vec<String>>,
}

/// Whether a pack is about a stack or a concern.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum PackKind {
    /// About a stack (Rust, React, Flutter...).
    Stack,
    /// About a concern across stacks (OWASP, accessibility, performance...).
    Concern,
}

/// A pack depends on another pack (within a version range) or on a skill, never on both at once.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged, rename_all_fields = "camelCase")]
pub enum PackDependency {
    /// A dependency on another pack.
    Pack {
        /// The pack's id.
        pack_id: String,
        /// The versions accepted, as a semver range.
        version_range: String,
    },
    /// A dependency on a skill.
    Skill {
        /// The skill's id.
        skill_id: String,
    },
}

/// A pack of rules and knowledge a review consumes; it is data and never executes code.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReviewPack {
    /// Namespaced identifier, such as `debuggatha/rust`.
    pub id: String,
    /// Semantic version.
    pub version: String,
    /// Stack or concern.
    pub kind: PackKind,
    /// Name shown to people.
    pub display_name: String,
    /// Its rules.
    pub rules: Vec<Rule>,
    /// The knowledge the rules derive from.
    pub knowledge: Vec<KnowledgeEntry>,
    /// What it needs to be present.
    pub depends_on: Vec<PackDependency>,
}

/// The tier a skill belongs to; the tiers are not one flat list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum SkillTier {
    /// Always runs, deterministic, model-independent.
    Core,
    /// Produces judgment for a scope.
    Review,
    /// Discovers; does not judge.
    Analysis,
}

/// A capability Debuggatha has, as the registry lists it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SkillDescriptor {
    /// Stable identifier.
    pub id: String,
    /// Its tier.
    pub tier: SkillTier,
    /// What it does.
    pub description: String,
}
