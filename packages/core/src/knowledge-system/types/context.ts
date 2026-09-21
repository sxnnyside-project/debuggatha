import type { RepositoryContext } from "../../repository-intelligence/index.js";
import type { ReviewRequest } from "../../review-engine/index.js";
import type { ReviewPolicy } from "./policy.js";

/**
 * The one shape every Skill receives, regardless of tier or `ReviewSource`
 * (Architecture doc §4 "Context Assembly"). Core Skills receive
 * `policy: undefined` — they run before any Pack is selected, per
 * CLAUDE.md "never let a Review Skill run before Core Skills have
 * oriented on the repo". Review/Analysis Skills receive a resolved
 * `ReviewPolicy`.
 */
export interface SkillContext {
  repository: RepositoryContext;
  request: ReviewRequest;
  policy: ReviewPolicy | undefined;
}
