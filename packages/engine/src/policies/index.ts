import type { RepositoryContext, ReviewRequest } from "@debuggatha/core";
import {
  type CapabilityRegistry,
  createInMemoryRegistry,
  type ReviewPolicy,
  resolvePolicy,
} from "@debuggatha/core";
import { reviewPacks } from "@debuggatha/packs";

export type { CapabilityRegistry, ReviewPolicy } from "@debuggatha/core";

/**
 * The one populated `CapabilityRegistry` instance for v1 (ADR-0004),
 * constructed once at module load from `@debuggatha/packs`'
 * static catalog. `engine/skills`' Review Skill functions are real
 *, but that package still doesn't export a `SkillDescriptor`
 * catalog — so no skills are registered here; a pack declaring a
 * `{ skillId }` dependency will still fail closed until that catalog
 * exists (see `core/knowledge-system`'s README "Risks").
 */
export const defaultCapabilityRegistry: CapabilityRegistry = createInMemoryRegistry(
  [...reviewPacks],
  [],
);

/**
 * Assembles a Review Policy — the specific, citable combination of one or
 * more Review Packs' Rules plus the repository's own resolved criteria —
 * for a single review run (see CLAUDE.md "Review Packs"). Thin wiring
 * only: the algorithm lives in `core/knowledge-system`'s
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
