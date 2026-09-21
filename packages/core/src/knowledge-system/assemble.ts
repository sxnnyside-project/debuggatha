import type { RepositoryContext } from "../repository-intelligence/index.js";
import type { ReviewRequest } from "../review-engine/index.js";
import type { SkillContext } from "./types/context.js";
import type { ReviewPolicy } from "./types/policy.js";

/**
 * Pure: `(RepositoryContext, ReviewRequest, ReviewPolicy | undefined) →
 * SkillContext` (Architecture doc §4 "Context Assembly"). Core Skills are
 * assembled with `policy: undefined` — see `SkillContext`'s doc comment.
 *
 * Only the freshly-built container is frozen (`Object.freeze`, shallow) —
 * `repository` is already deep-frozen by repository-intelligence and
 * `request`/`policy` are caller-owned values this function doesn't clone,
 * so recursively freezing into them here would be a surprising side
 * effect on objects this function doesn't own.
 */
export function assembleContext(
  repository: RepositoryContext,
  request: ReviewRequest,
  policy: ReviewPolicy | undefined,
): SkillContext {
  return Object.freeze({ repository, request, policy });
}
