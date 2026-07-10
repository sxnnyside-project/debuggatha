import type { Category, ReviewPackReference, Severity } from "@debuggatha/review-engine";

/**
 * Where a `PolicyRule` came from — a Pack's `Rule` or the repository's own
 * `CriteriaRule` (repository-intelligence). Rule Resolution tags every
 * rule it merges with this provenance (Architecture doc §3 domain-model
 * table: "tagging each with provenance").
 */
export type PolicyRuleOrigin =
  | { kind: "pack"; packId: string; packVersion: string }
  | { kind: "criteria" };

/**
 * The unified shape `ReviewPolicy.rules` carries, regardless of whether it
 * came from a Review Pack's `Rule` or a repository's own `CriteriaRule`.
 * `category`/`defaultSeverity` are `undefined` for criteria-derived
 * entries: `CriteriaRule` (repository-intelligence, frozen since Epic 1)
 * does not classify by category or severity — leaving these `undefined`
 * instead of guessing a value is CLAUDE.md's "never fakes certainty"
 * applied to policy composition, not just to Findings.
 */
export interface PolicyRule {
  id: string;
  statement: string;
  category: Category | undefined;
  defaultSeverity: Severity | undefined;
  origin: PolicyRuleOrigin;
}

/**
 * A detected-and-resolved conflict (ADR-0003). The losing rule never
 * appears in `ReviewPolicy.rules`, but this record keeps it traceable —
 * "the losing rule is never silently dropped."
 */
export interface ConflictRecord {
  winningRuleId: string;
  losingRuleId: string;
  reason: string;
}

/**
 * What a `ReviewPolicyReference { id }` (review-engine) points at once
 * resolved. `id` is a deterministic hash of its resolution inputs
 * (ADR-0005) — two `resolvePolicy` calls with identical inputs produce
 * identical ids.
 */
export interface ReviewPolicy {
  id: string;
  rules: PolicyRule[];
  packRefs: ReviewPackReference[];
  conflicts: ConflictRecord[];
}
