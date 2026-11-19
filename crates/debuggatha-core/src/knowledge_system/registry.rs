//! The registry of packs and skills a review resolves against.

use std::collections::{HashMap, HashSet};

use super::pack::{PackDependency, PackKind, ReviewPack, SkillDescriptor, SkillTier};
use super::semver::{compare_versions, satisfies_range};

/// The packs a request resolved to, and the dependencies that could not be met.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedDependencies {
    /// Every pack needed, dependencies first.
    pub resolved: Vec<ReviewPack>,
    /// Dependencies no registered pack or skill satisfies.
    pub missing: Vec<PackDependency>,
}

/// What packs and skills are known, and which pack satisfies a request. In memory: the engine
/// loads its packs at start.
#[derive(Debug, Clone, Default)]
pub struct CapabilityRegistry {
    packs_by_id: HashMap<String, Vec<ReviewPack>>,
    all_packs: Vec<ReviewPack>,
    all_skills: Vec<SkillDescriptor>,
    skill_ids: HashSet<String>,
}

impl CapabilityRegistry {
    /// A registry with these packs and skills.
    #[must_use]
    pub fn new(packs: Vec<ReviewPack>, skills: Vec<SkillDescriptor>) -> Self {
        let mut registry = Self::default();
        for pack in packs {
            registry.register_pack(pack);
        }
        for skill in skills {
            registry.register_skill(skill);
        }
        registry
    }

    /// Registering the same id and version again replaces the earlier pack.
    pub fn register_pack(&mut self, pack: ReviewPack) {
        let versions = self.packs_by_id.entry(pack.id.clone()).or_default();
        match versions.iter().position(|p| p.version == pack.version) {
            Some(at) => versions[at] = pack.clone(),
            None => versions.push(pack.clone()),
        }
        // A re-registered pack replaces the earlier one in the list (docs/PARITY.md).
        match self
            .all_packs
            .iter()
            .position(|p| p.id == pack.id && p.version == pack.version)
        {
            Some(at) => self.all_packs[at] = pack,
            None => self.all_packs.push(pack),
        }
    }

    /// Register a skill, replacing one with the same id.
    pub fn register_skill(&mut self, skill: SkillDescriptor) {
        self.skill_ids.insert(skill.id.clone());
        self.all_skills.push(skill);
    }

    /// The skills, of one tier or all of them.
    #[must_use]
    pub fn list_skills(&self, tier: Option<SkillTier>) -> Vec<&SkillDescriptor> {
        self.all_skills
            .iter()
            .filter(|s| tier.is_none_or(|t| s.tier == t))
            .collect()
    }

    /// The packs, of one kind or all of them.
    #[must_use]
    pub fn list_packs(&self, kind: Option<PackKind>) -> Vec<&ReviewPack> {
        self.all_packs
            .iter()
            .filter(|p| kind.is_none_or(|k| p.kind == k))
            .collect()
    }

    /// The highest version of `id` that satisfies `version_range` (any version when `None`).
    /// Ties keep the one registered first.
    #[must_use]
    pub fn get_pack(&self, id: &str, version_range: Option<&str>) -> Option<&ReviewPack> {
        let mut best: Option<&ReviewPack> = None;
        for candidate in self.packs_by_id.get(id).into_iter().flatten() {
            if version_range.is_some_and(|range| !satisfies_range(&candidate.version, range)) {
                continue;
            }
            if best.is_none_or(|b| compare_versions(&candidate.version, &b.version).is_gt()) {
                best = Some(candidate);
            }
        }
        best
    }

    /// The requested packs and everything they depend on, in the order first reached, plus what
    /// could not be found. A missing dependency is reported, never skipped silently.
    #[must_use]
    pub fn resolve_dependencies(&self, pack_ids: &[String]) -> ResolvedDependencies {
        let mut resolved: Vec<ReviewPack> = Vec::new();
        let mut missing = Vec::new();
        let mut visited = HashSet::new();
        for id in pack_ids {
            self.visit(id, None, &mut resolved, &mut missing, &mut visited);
        }
        ResolvedDependencies { resolved, missing }
    }

    fn visit(
        &self,
        pack_id: &str,
        version_range: Option<&str>,
        resolved: &mut Vec<ReviewPack>,
        missing: &mut Vec<PackDependency>,
        visited: &mut HashSet<String>,
    ) {
        if !visited.insert(format!("{pack_id}@{}", version_range.unwrap_or("*"))) {
            return;
        }
        let Some(pack) = self.get_pack(pack_id, version_range) else {
            missing.push(PackDependency::Pack {
                pack_id: pack_id.to_owned(),
                version_range: version_range.unwrap_or("*").to_owned(),
            });
            return;
        };
        // A pack reached twice at different versions keeps its first position and the later version.
        match resolved.iter().position(|p| p.id == pack.id) {
            Some(at) => resolved[at] = pack.clone(),
            None => resolved.push(pack.clone()),
        }
        for dependency in &pack.depends_on {
            match dependency {
                PackDependency::Skill { skill_id } => {
                    if !self.skill_ids.contains(skill_id) {
                        missing.push(dependency.clone());
                    }
                }
                PackDependency::Pack {
                    pack_id,
                    version_range,
                } => {
                    self.visit(pack_id, Some(version_range), resolved, missing, visited);
                }
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)] // a failed expect is a failed test
mod tests {
    use super::{CapabilityRegistry, PackKind, SkillTier};
    use crate::knowledge_system::{ReviewPack, SkillDescriptor};

    fn pack(id: &str, version: &str, kind: &str) -> ReviewPack {
        serde_json::from_value(serde_json::json!({
            "id": id, "version": version, "kind": kind, "displayName": id,
            "rules": [], "knowledge": [], "dependsOn": []
        }))
        .expect("a pack")
    }

    fn skill(id: &str, tier: &str) -> SkillDescriptor {
        serde_json::from_value(serde_json::json!({ "id": id, "tier": tier, "description": id }))
            .expect("a skill")
    }

    #[test]
    fn packs_and_skills_list_by_kind_and_tier_and_a_replaced_pack_is_not_listed() {
        let registry = CapabilityRegistry::new(
            vec![
                pack("a", "1.0.0", "stack"),
                pack("b", "1.0.0", "concern"),
                pack("a", "1.0.0", "stack"),
            ],
            vec![skill("s1", "core"), skill("s2", "review")],
        );
        assert_eq!(registry.list_packs(None).len(), 2);
        assert_eq!(registry.list_packs(Some(PackKind::Concern)).len(), 1);
        assert_eq!(registry.list_skills(None).len(), 2);
        assert_eq!(registry.list_skills(Some(SkillTier::Core))[0].id, "s1");
    }
}
