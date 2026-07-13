import type { Finding } from "@debuggatha/review-engine";
import { acceptedDeviationMatches, exceptionMatches, suppressionMatches } from "./matching.js";
import { isActiveMemoryStatus } from "./types/lifecycle.js";
import type { MemoryStore } from "./types/store.js";

export interface SuppressedFinding {
  finding: Finding;
  memoryItemId: string;
  reason: "suppression" | "accepted-deviation" | "exception";
}

export interface FilterFindingsResult {
  findings: Finding[];
  suppressed: SuppressedFinding[];
}

/**
 * The automatic-consumption seam Epic 15 asks for: "future reviews
 * should consume Repository Memory automatically, no manual intervention
 * required." Only `"active"` memory items are ever consulted — a
 * `"detected"` or `"suggested"` suppression a user hasn't confirmed yet
 * must never silently hide a real finding ("never present inferred
 * knowledge as confirmed fact"). Every suppressed finding is reported in
 * `suppressed`, not just dropped — "suppression should remain explicit
 * and traceable... avoid hidden ignore lists."
 */
export function filterSuppressedFindings(
  findings: Finding[],
  store: MemoryStore,
): FilterFindingsResult {
  const activeSuppressions = store.items.filter(
    (item): item is Extract<typeof item, { category: "suppression" }> =>
      item.category === "suppression" && isActiveMemoryStatus(item.status),
  );
  const activeDeviations = store.items.filter(
    (item): item is Extract<typeof item, { category: "accepted-deviation" }> =>
      item.category === "accepted-deviation" && isActiveMemoryStatus(item.status),
  );
  const activeExceptions = store.items.filter(
    (item): item is Extract<typeof item, { category: "exception" }> =>
      item.category === "exception" && isActiveMemoryStatus(item.status),
  );

  const kept: Finding[] = [];
  const suppressed: SuppressedFinding[] = [];

  for (const finding of findings) {
    const suppression = activeSuppressions.find((item) => suppressionMatches(item, finding));
    if (suppression) {
      suppressed.push({ finding, memoryItemId: suppression.id, reason: "suppression" });
      continue;
    }
    const deviation = activeDeviations.find((item) => acceptedDeviationMatches(item, finding));
    if (deviation) {
      suppressed.push({ finding, memoryItemId: deviation.id, reason: "accepted-deviation" });
      continue;
    }
    const exception = activeExceptions.find((item) => exceptionMatches(item, finding));
    if (exception) {
      suppressed.push({ finding, memoryItemId: exception.id, reason: "exception" });
      continue;
    }
    kept.push(finding);
  }

  return { findings: kept, suppressed };
}
