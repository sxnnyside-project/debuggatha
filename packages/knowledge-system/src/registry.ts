import { compareVersions, satisfiesRange } from "./internal/semver.js";
import type { PackDependency, ReviewPack, ReviewPackKind } from "./types/pack.js";
import type { SkillDescriptor, SkillTier } from "./types/skill.js";

/**
 * Answers "what exists and what does it need" — nothing else. Never
 * executes anything, never sees a `RepositoryContext` (ADR-0004).
 * Injectable, not a global singleton — same discipline as
 * `RepositoryContextCache` in Epic 1.
 */
export interface CapabilityRegistry {
  registerSkill(skill: SkillDescriptor): void;
  registerPack(pack: ReviewPack): void;
  listSkills(tier?: SkillTier): SkillDescriptor[];
  listPacks(kind?: ReviewPackKind): ReviewPack[];
  getPack(id: string, versionRange?: string): ReviewPack | undefined;
  resolveDependencies(packIds: string[]): {
    resolved: ReviewPack[];
    missing: PackDependency[];
  };
}

function highestVersion(packs: ReviewPack[]): ReviewPack | undefined {
  return packs.reduce<ReviewPack | undefined>((best, candidate) => {
    if (!best) return candidate;
    return compareVersions(candidate.version, best.version) > 0 ? candidate : best;
  }, undefined);
}

/**
 * Static in-memory implementation — v1's whole catalog (`@debuggatha/policies`
 * populates it from `@debuggatha/review-packs` + `@debuggatha/skills` at
 * module load). No filesystem scanning, no network fetch, no dynamic
 * registration API (ADR-0004).
 */
export function createInMemoryRegistry(
  initialPacks: ReviewPack[] = [],
  initialSkills: SkillDescriptor[] = [],
): CapabilityRegistry {
  const packsById = new Map<string, ReviewPack[]>();
  const allPacks: ReviewPack[] = [];
  const allSkills: SkillDescriptor[] = [];
  const skillIds = new Set<string>();

  function registerPack(pack: ReviewPack): void {
    const existing = packsById.get(pack.id) ?? [];
    // Replace if same version, else add
    const index = existing.findIndex((p) => p.version === pack.version);
    if (index >= 0) {
      existing[index] = pack;
    } else {
      existing.push(pack);
    }
    packsById.set(pack.id, existing);
    allPacks.push(pack);
  }

  function registerSkill(skill: SkillDescriptor): void {
    allSkills.push(skill);
    skillIds.add(skill.id);
  }

  for (const pack of initialPacks) registerPack(pack);
  for (const skill of initialSkills) registerSkill(skill);

  function getPack(id: string, versionRange?: string): ReviewPack | undefined {
    const candidates = packsById.get(id) ?? [];
    if (!versionRange) return highestVersion(candidates);
    return highestVersion(candidates.filter((pack) => satisfiesRange(pack.version, versionRange)));
  }

  function resolveDependencies(packIds: string[]): {
    resolved: ReviewPack[];
    missing: PackDependency[];
  } {
    const resolved = new Map<string, ReviewPack>();
    const missing: PackDependency[] = [];
    const visited = new Set<string>();

    function visit(packId: string, versionRange?: string): void {
      const visitKey = `${packId}@${versionRange ?? "*"}`;
      if (visited.has(visitKey)) return;
      visited.add(visitKey);

      const pack = getPack(packId, versionRange);
      if (!pack) {
        missing.push({ packId, versionRange: versionRange ?? "*" });
        return;
      }
      resolved.set(pack.id, pack);

      for (const dependency of pack.dependsOn) {
        if ("skillId" in dependency) {
          if (!skillIds.has(dependency.skillId)) {
            missing.push(dependency);
          }
        } else {
          visit(dependency.packId, dependency.versionRange);
        }
      }
    }

    for (const packId of packIds) {
      visit(packId);
    }

    return { resolved: [...resolved.values()], missing };
  }

  return {
    registerSkill,
    registerPack,
    listSkills: (tier) =>
      tier ? allSkills.filter((skill) => skill.tier === tier) : [...allSkills],
    listPacks: (kind) => (kind ? allPacks.filter((pack) => pack.kind === kind) : [...allPacks]),
    getPack,
    resolveDependencies,
  };
}
