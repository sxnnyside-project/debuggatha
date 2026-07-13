import {
  type CapabilityRegistry,
  createInMemoryRegistry,
  type ReviewPolicy,
  resolvePolicy,
} from "@debuggatha/knowledge-system";
import type { RepositoryContext } from "@debuggatha/repository-intelligence";
import type { ReviewRequest } from "@debuggatha/review-engine";
import { reviewPacks } from "@debuggatha/review-packs";

export type { CapabilityRegistry, ReviewPolicy } from "@debuggatha/knowledge-system";

/**
 * The one populated `CapabilityRegistry` instance for v1 (ADR-0004),
 * constructed once at module load from `@debuggatha/review-packs`'
 * static catalog. `@debuggatha/skills`' Review Skill functions are real
 * (Epic 11), but that package still doesn't export a `SkillDescriptor`
 * catalog — so no skills are registered here; a pack declaring a
 * `{ skillId }` dependency will still fail closed until that catalog
 * exists (see `@debuggatha/knowledge-system`'s README "Risks").
 */
export const defaultCapabilityRegistry: CapabilityRegistry = createInMemoryRegistry(
  [...reviewPacks],
  [],
);

/**
 * Assembles a Review Policy — the specific, citable combination of one or
 * more Review Packs' Rules plus the repository's own resolved criteria —
 * for a single review run (see CLAUDE.md "Review Packs"). Thin wiring
 * only: the algorithm lives in `@debuggatha/knowledge-system`'s
 * `resolvePolicy`; this function's whole job is supplying the populated
 * registry (Architecture doc §4 "Policies").
 */
export function assemblePolicy(
  context: RepositoryContext,
  request: ReviewRequest,
  registry: CapabilityRegistry = defaultCapabilityRegistry,
): ReviewPolicy {
  return resolvePolicy(context, request, registry);
}
