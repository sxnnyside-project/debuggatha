import type { ReviewPolicy } from "@debuggatha/knowledge-system";
import type { Finding } from "@debuggatha/review-engine";
import { readSourceUnits } from "../engine/read-files.js";
import { runReviewSkillsEngine } from "../engine/run.js";

/**
 * Review Skill: File Review.
 *
 * Reviews one or more explicit files in full — the editor-integration
 * scope (e.g. "review the file I have open"), as opposed to Diff Review
 * (only changed lines) or Architecture Review (the whole tree).
 */
export function reviewFiles(paths: string[], policy: ReviewPolicy): Finding[] {
  const units = readSourceUnits(paths);
  return runReviewSkillsEngine(policy, units);
}
