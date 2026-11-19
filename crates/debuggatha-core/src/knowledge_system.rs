//! Review Packs, the capability registry and policy resolution (port of `core/knowledge-system`).

mod collate;
mod pack;
mod registry;
mod resolve;
mod semver;
mod validate;

pub use collate::locale_compare;
pub use pack::{
    KnowledgeEntry, PackDependency, PackKind, ReviewPack, Rule, RuleScope, SkillDescriptor,
    SkillTier,
};
pub use registry::{CapabilityRegistry, ResolvedDependencies};
pub use resolve::{
    ConflictRecord, PolicyError, PolicyRequest, PolicyRule, PolicyRuleOrigin, ResolutionContext,
    ReviewPolicy, resolve_policy,
};
pub use semver::{compare_versions, satisfies_range};
pub use validate::{PackValidationError, validate_review_pack};

/// Part of every policy id: bump it when the id derivation changes (ADR-0005).
pub const KNOWLEDGE_SCHEMA_VERSION: &str = "1";
