import type { ReviewPolicy } from "@debuggatha/knowledge-system";
import type { Finding } from "@debuggatha/review-engine";

/**
 * Review Skill: Architecture Review.
 *
 * Detects the architectural pattern in use (Clean Architecture, Hexagonal,
 * MVC, MVVM, Feature-First, Layered) and adapts its critique to that
 * pattern instead of applying a generic checklist. Deferred past the v1
 * proof-of-pipeline — see CLAUDE.md "Deferred (not v1)".
 *
 * `ReviewPolicy` is `@debuggatha/knowledge-system`'s resolved policy
 * (`{ id, rules, packRefs, conflicts }`), not review-engine's bare
 * `ReviewPolicyReference { id }` — see docs/adr/0001 and Architecture doc
 * §4 "Skills".
 */
export function reviewArchitecture(_rootDir: string, _policy: ReviewPolicy): Finding[] {
  return []; // Deferred implementation
}
