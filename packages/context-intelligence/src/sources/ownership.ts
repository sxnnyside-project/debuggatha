import { detectOwnership, inferModuleBoundaries } from "@debuggatha/analysis-engine";
import type { OwnershipContextItem } from "../types.js";

/**
 * Ownership context (Epic 14): reuses
 * `@debuggatha/analysis-engine`'s `inferModuleBoundaries`/
 * `detectOwnership` (Epic 12) directly rather than re-implementing
 * CODEOWNERS parsing here — "do not duplicate Analysis Engine". Only
 * module boundaries the Analysis Engine could confidently attribute
 * (`source !== "unknown"`) become a `ContextItem` at all — an
 * unattributed module is simply absent from this list, never reported
 * with a fabricated or empty-owners claim (Epic 14: "ownership remains
 * contextual information... never opine without evidence").
 */
export function buildOwnershipItems(rootDir: string): OwnershipContextItem[] {
  const boundaries = inferModuleBoundaries(rootDir);
  const signals = detectOwnership(rootDir, boundaries);

  return signals
    .filter((signal) => signal.source !== "unknown")
    .map((signal) => ({
      id: `ownership:${signal.moduleId}`,
      category: "ownership",
      confidence: "documented",
      evidence: [
        {
          file: "CODEOWNERS",
          detail: `Owners for "${signal.moduleId}": ${signal.owners.join(", ")}`,
        },
      ],
      source: "CODEOWNERS",
      moduleId: signal.moduleId,
      owners: signal.owners,
    }));
}
