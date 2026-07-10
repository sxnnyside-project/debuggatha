import type { ReviewPolicy } from "@debuggatha/knowledge-system";
import type { Finding } from "@debuggatha/review-engine";

/**
 * Review Skill: Diff Review.
 *
 * Applies an assembled Review Policy to what changed since the last commit
 * or against a base branch — the default review mode (see CLAUDE.md "MCP
 * surface", `review_diff`). Deferred past the v1 proof-of-pipeline.
 *
 * `ReviewPolicy` is `@debuggatha/knowledge-system`'s resolved policy
 * (`{ id, rules, packRefs, conflicts }`), not review-engine's bare
 * `ReviewPolicyReference { id }` — see docs/adr/0001 and Architecture doc
 * §4 "Skills".
 */
export function reviewDiff(_diff: string, _policy: ReviewPolicy): Finding[] {
  return []; // Deferred implementation
}
