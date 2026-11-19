use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::review_engine::{Category, Severity};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum RuleScope {
    Always,
    RequiresFramework { framework: String },
    RequiresLanguage { language: String },
    Glob { pattern: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgeEntry {
    pub id: String,
    pub title: String,
    pub body: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub external_refs: Option<Vec<String>>,
    pub limitations: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Rule {
    pub id: String,
    pub pack_id: String,
    pub statement: String,
    pub category: Category,
    pub applies_to: RuleScope,
    pub default_severity: Severity,
    pub knowledge_refs: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contradicts: Option<Vec<String>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum PackKind {
    Stack,
    Concern,
}

/// A pack depends on another pack (within a version range) or on a skill, never on both at once.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged, rename_all_fields = "camelCase")]
pub enum PackDependency {
    Pack {
        pack_id: String,
        version_range: String,
    },
    Skill {
        skill_id: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReviewPack {
    pub id: String,
    pub version: String,
    pub kind: PackKind,
    pub display_name: String,
    pub rules: Vec<Rule>,
    pub knowledge: Vec<KnowledgeEntry>,
    pub depends_on: Vec<PackDependency>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum SkillTier {
    Core,
    Review,
    Analysis,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SkillDescriptor {
    pub id: String,
    pub tier: SkillTier,
    pub description: String,
}
