import type { Finding, ReviewPolicy } from "@debuggatha/core";
import { parseUnifiedDiff } from "../engine/diff-parser.js";
import { type RunOptions, runReviewSkillsEngine } from "../engine/run.js";

/**
 * Review Skill: Diff Review.
 *
 * Applies an assembled Review Policy to what changed since the last
 * commit or against a base branch — the default review mode (see
 * CLAUDE.md "MCP surface", `review_diff`). Only the *added* lines of the
 * diff are scanned (see `parseUnifiedDiff`), never the rest of the
 * touched files — this is what keeps a Diff Review from turning into a
 * full workspace sweep.
 *
 * `ReviewPolicy` is `core/knowledge-system`'s resolved policy
 * (`{ id, rules, packRefs, conflicts }`), not review-engine's bare
 * `ReviewPolicyReference { id }` — see docs/adr/0001 and Architecture doc
 * §4 "Skills".
 */
export function reviewDiff(diff: string, policy: ReviewPolicy, options?: RunOptions): Finding[] {
  const units = parseUnifiedDiff(diff);
  return runReviewSkillsEngine(policy, units, options);
}
