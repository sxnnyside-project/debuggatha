import type { ContextItem } from "./types.js";

/**
 * A light, optional integration seam (Epic 14 "Integration": "Review
 * Skills, Runtime Engine... should all consume the same context model").
 * Turns `"engineering-convention"` items back into plain statement
 * strings — exactly the shape
 * `@debuggatha/runtime-engine`'s `policyStatements` parameter already
 * expects (see that package's `buildSemanticPrompt`). This is the only
 * "interpretation" this package does, and it's trivial by construction:
 * it doesn't decide what a convention *means*, it just hands the
 * already-extracted rule text to whatever consumes policy statements —
 * the actual judgment stays downstream, in the Runtime Engine/Review
 * Skill that receives it.
 */
export function conventionsToPolicyStatements(items: ContextItem[]): string[] {
  return items
    .filter(
      (item): item is Extract<ContextItem, { category: "engineering-convention" }> =>
        item.category === "engineering-convention",
    )
    .map((item) => item.rule);
}
