import type { PackDependency, ReviewPack } from "./types/pack.js";

/**
 * Structural well-formedness only — not semantic correctness of a rule's
 * `statement` (Architecture doc §11: NLP-based content checking is
 * explicitly out of scope). Pure: no filesystem/network I/O, throws a
 * single aggregated `Error` naming every issue found rather than stopping
 * at the first one, so a pack author fixing a pack sees the whole list at
 * once.
 *
 * Per ADR-0002's amendment, a pack `id` has no reserved delimiter to
 * avoid — the `"packId@version"` string-encoding concern the original
 * proposal raised no longer applies now that `ReviewPackReference.version`
 * is a real field, so this function does not check `id` for reserved
 * characters.
 */
export function validateReviewPack(pack: ReviewPack): void {
  const issues: string[] = [];

  if (!pack.id.trim()) {
    issues.push("pack.id must be a non-empty string");
  }
  if (!/^\d+\.\d+\.\d+/.test(pack.version.trim())) {
    issues.push(
      `pack.version "${pack.version}" is not a semver-shaped string (expected "x.y.z...")`,
    );
  }

  const ruleIds = new Set<string>();
  for (const rule of pack.rules) {
    if (ruleIds.has(rule.id)) {
      issues.push(`duplicate rule id "${rule.id}" within pack "${pack.id}"`);
    }
    ruleIds.add(rule.id);

    if (rule.packId !== pack.id) {
      issues.push(
        `rule "${rule.id}" declares packId "${rule.packId}", which does not match its containing pack "${pack.id}"`,
      );
    }

    if (rule.knowledgeRefs.length === 0) {
      issues.push(
        `rule "${rule.id}" has no knowledgeRefs — every Rule must cite at least one KnowledgeEntry`,
      );
    }
    for (const ref of rule.knowledgeRefs) {
      if (!pack.knowledge.some((entry) => entry.id === ref)) {
        issues.push(`rule "${rule.id}" cites unknown knowledgeRefs entry "${ref}"`);
      }
    }
  }

  const knowledgeIds = new Set<string>();
  for (const entry of pack.knowledge) {
    if (knowledgeIds.has(entry.id)) {
      issues.push(`duplicate knowledge entry id "${entry.id}" within pack "${pack.id}"`);
    }
    knowledgeIds.add(entry.id);
  }

  for (const dependency of pack.dependsOn) {
    issues.push(...describeMalformedDependency(dependency));
  }

  if (issues.length > 0) {
    throw new Error(`Review pack "${pack.id}" failed validation:\n- ${issues.join("\n- ")}`);
  }
}

function describeMalformedDependency(dependency: PackDependency): string[] {
  const hasPackId = "packId" in dependency && Boolean(dependency.packId);
  const hasSkillId = "skillId" in dependency && Boolean(dependency.skillId);

  if (hasPackId && hasSkillId) {
    return [
      "dependsOn entry declares both packId and skillId — a dependency must be exactly one shape",
    ];
  }
  if (!hasPackId && !hasSkillId) {
    return ["dependsOn entry declares neither a packId nor a skillId"];
  }
  if (hasPackId && "packId" in dependency && !dependency.versionRange?.trim()) {
    return [`dependsOn entry for pack "${dependency.packId}" is missing a non-empty versionRange`];
  }
  return [];
}
