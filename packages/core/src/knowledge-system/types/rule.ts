import type { Category, Severity } from "../../review-engine/index.js";

/**
 * When a Rule is even relevant. Resolved against `RepositoryContext.stack`
 * at Rule Resolution time for `always` / `requires-framework` /
 * `requires-language`. `glob` is deliberately *not* resolved here — Rule
 * Resolution operates on `RepositoryContext.stack`, not a concrete file
 * list, so a glob-scoped rule always survives Rule Resolution unfiltered;
 * matching it against the files actually in a `ReviewRequest`'s scope is a
 * Skill's job at execution time (see package README "Technical decisions").
 */
export type RuleScope =
  | { kind: "always" }
  | { kind: "requires-framework"; framework: string }
  | { kind: "requires-language"; language: string }
  | { kind: "glob"; pattern: string };

export interface KnowledgeEntry {
  id: string;
  title: string;
  body: string;
  externalRefs: string[] | undefined;
  /**
   * Known heuristic limitations — scenarios where the Rule(s) citing this
   * entry produce a false positive or false negative (PACK_SPEC.md §4,
   * "MUST document known heuristic limitations"). Required, not optional:
   * every entry must name at least one concrete case where the rule it
   * backs does not hold, or explicitly state none is currently known.
   * Deliberately plain prose, not a machine-checkable exception list — the
   * Review Skill consuming this is still a human-facing citation, not an
   * automated suppression mechanism.
   */
  limitations: string[];
}

/**
 * Rules are declarative data, not executable code (Architecture doc §5 —
 * the Biome/GritQL lesson: start narrower). A Rule cannot decide "this
 * file violates me"; that judgment belongs to a Skill, which uses the
 * Rule as a citable constraint.
 */
export interface Rule {
  id: string;
  packId: string;
  statement: string;
  category: Category;
  appliesTo: RuleScope;
  defaultSeverity: Severity;
  /** Every Rule must cite at least one KnowledgeEntry — "no source, no finding" (CLAUDE.md). */
  knowledgeRefs: string[];
  /**
   * Opt-in conflict declaration (ADR-0003 decision #1): other rule ids
   * (in this pack, another pack, or a repository `CriteriaRule`) this
   * Rule is known to make an opposite claim to. No automatic semantic
   * contradiction detection is attempted — see ADR-0003 and Architecture
   * doc §11.
   */
  contradicts: string[] | undefined;
}
