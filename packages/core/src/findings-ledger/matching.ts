import type { Finding } from "../review-engine/index.js";
import type { LedgerEntry } from "./types/entry.js";
import { computeFindingFingerprint } from "./types/fingerprint.js";

/**
 * Finding Matching (epic §6) — "design for future improvements": this is
 * one pluggable strategy behind a stable interface, not the only
 * strategy this package will ever have. `defaultFindingMatcher` is
 * intentionally simple (structural: file + rule identity, with a content
 * hash as a secondary "did it change" signal) — a future matcher could
 * use AST diffing or embedding similarity without `synchronizeReviewResult`
 * or its callers changing at all.
 */
export type MatchOutcome = { kind: "matched"; entryId: string; changed: boolean } | { kind: "new" };

/**
 * `candidateEntries` is every entry in the ledger, not just active ones —
 * a resolved or dismissed entry must still be matchable, since matching
 * one is exactly what signals it should reopen (see `synchronizeReviewResult`).
 */
export type FindingMatcher = (candidate: Finding, candidateEntries: LedgerEntry[]) => MatchOutcome;

/**
 * Primary key: (file, ruleId) when the finding cites a rule; falls back
 * to (file, category) when it doesn't (e.g. a Core Skill finding with no
 * Review Pack / criteria rule behind it). Never keys on line number alone
 * — line numbers shift when unrelated code above a finding changes.
 * `changed` is only computable when both sides have a content anchor;
 * otherwise a match is reported unchanged (no basis to claim otherwise).
 */
export const defaultFindingMatcher: FindingMatcher = (candidate, candidateEntries) => {
  const candidateFp = computeFindingFingerprint(candidate);

  for (const entry of candidateEntries) {
    if (entry.fingerprint.file !== candidateFp.file) continue;

    const sameIdentity =
      candidateFp.ruleId !== undefined
        ? entry.fingerprint.ruleId === candidateFp.ruleId
        : entry.fingerprint.ruleId === undefined &&
          entry.fingerprint.category === candidateFp.category;

    if (!sameIdentity) continue;

    const changed =
      entry.fingerprint.contentAnchor !== undefined &&
      candidateFp.contentAnchor !== undefined &&
      entry.fingerprint.contentAnchor !== candidateFp.contentAnchor;

    return { kind: "matched", entryId: entry.id, changed };
  }

  return { kind: "new" };
};
