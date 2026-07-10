import type { KnowledgeEntry, Rule } from "./rule.js";

export type ReviewPackKind = "stack" | "concern";

/**
 * A hard dependency (§9): either on another pack at a version range, or on
 * a Skill being registered. No soft/optional dependencies in v1 — see
 * Architecture doc §11.
 */
export type PackDependency = { packId: string; versionRange: string } | { skillId: string };

/**
 * Content, not logic (Architecture doc §4 "Review Packs"). A `ReviewPack`
 * is the schema `@debuggatha/review-packs` conforms its content to — the
 * Terraform-provider-schema / ESLint-plugin lesson from Architecture doc
 * §2: the schema is inspectable without executing anything.
 */
export interface ReviewPack {
  id: string;
  version: string;
  kind: ReviewPackKind;
  displayName: string;
  rules: Rule[];
  knowledge: KnowledgeEntry[];
  dependsOn: PackDependency[];
}
