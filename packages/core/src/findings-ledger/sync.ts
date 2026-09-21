import type { RepositoryContext } from "../repository-intelligence/index.js";
import type { ReviewResult } from "../review-engine/index.js";
import { deepFreeze } from "./internal/deep-freeze.js";
import { defaultFindingMatcher, type FindingMatcher } from "./matching.js";
import { associationFor } from "./types/association.js";
import { createLedgerEntry, refreshLedgerEntry, transitionFinding } from "./types/entry.js";
import { computeFindingFingerprint } from "./types/fingerprint.js";
import type { Ledger } from "./types/ledger.js";
import { isActiveFindingStatus } from "./types/lifecycle.js";

/**
 * What files this review actually looked at. NOT inferred from
 * `ReviewRequest.scope` — a "diff" scope's touched-file list requires git
 * knowledge this package deliberately doesn't have (see README "Technical
 * decisions"). The caller (a future orchestrator) supplies it explicitly.
 * `"workspace"` means "the whole repository was in scope" — every active
 * entry becomes an auto-resolve candidate if not re-detected.
 */
export type SyncScope = { kind: "workspace" } | { kind: "files"; files: string[] };

export interface SyncReport {
  newEntryIds: string[];
  unchangedEntryIds: string[];
  changedEntryIds: string[];
  autoResolvedEntryIds: string[];
  reopenedEntryIds: string[];
}

function inScope(file: string, scope: SyncScope): boolean {
  return scope.kind === "workspace" || scope.files.includes(file);
}

/**
 * Reconciles a fresh `ReviewResult` into the `Ledger` (epic §6, §9).
 * Pure: returns a new `Ledger`, never mutates the one passed in. Never
 * resolves an entry whose file falls outside `scope` — a File Review of
 * one file must not silently resolve findings elsewhere the review never
 * looked at.
 */
export function synchronizeReviewResult(
  ledger: Ledger,
  result: ReviewResult,
  context: RepositoryContext,
  scope: SyncScope,
  options: { matcher?: FindingMatcher } = {},
): { ledger: Ledger; report: SyncReport } {
  const matcher = options.matcher ?? defaultFindingMatcher;
  const association = associationFor(result, context);

  const entriesById = new Map(ledger.entries.map((entry) => [entry.id, entry]));
  const touchedThisRun = new Set<string>();

  const report: SyncReport = {
    newEntryIds: [],
    unchangedEntryIds: [],
    changedEntryIds: [],
    autoResolvedEntryIds: [],
    reopenedEntryIds: [],
  };

  for (const finding of result.findings) {
    // Matching candidates are *every* entry, not just active ones — a
    // resolved or dismissed entry must still be matchable, since matching
    // one is exactly what triggers reopening it below. Only the
    // auto-resolve pass further down is restricted to active entries.
    const candidateEntries = [...entriesById.values()];
    const outcome = matcher(finding, candidateEntries);
    const fingerprint = computeFindingFingerprint(finding);

    if (outcome.kind === "new") {
      const entry = createLedgerEntry({
        fingerprint,
        finding,
        review: association,
        origin: { kind: "review", sessionId: result.sessionId },
      });
      entriesById.set(entry.id, entry);
      touchedThisRun.add(entry.id);
      report.newEntryIds.push(entry.id);
      continue;
    }

    const existing = entriesById.get(outcome.entryId);
    if (!existing) continue; // unreachable: matcher only returns ids it was given

    touchedThisRun.add(existing.id);

    if (existing.status === "resolved" || existing.status === "dismissed") {
      const reopened = transitionFinding(existing, "reopened", {
        kind: "review",
        sessionId: result.sessionId,
      });
      const refreshed = refreshLedgerEntry(reopened, finding, fingerprint, association);
      entriesById.set(refreshed.id, refreshed);
      report.reopenedEntryIds.push(refreshed.id);
    } else {
      const refreshed = refreshLedgerEntry(existing, finding, fingerprint, association);
      entriesById.set(refreshed.id, refreshed);
      (outcome.changed ? report.changedEntryIds : report.unchangedEntryIds).push(refreshed.id);
    }
  }

  for (const entry of entriesById.values()) {
    if (!isActiveFindingStatus(entry.status)) continue;
    if (touchedThisRun.has(entry.id)) continue;
    if (!inScope(entry.fingerprint.file, scope)) continue;

    const resolved = transitionFinding(entry, "resolved", {
      kind: "review",
      sessionId: result.sessionId,
    });
    entriesById.set(resolved.id, resolved);
    report.autoResolvedEntryIds.push(resolved.id);
  }

  const updatedLedger: Ledger = deepFreeze({
    ...ledger,
    entries: [...entriesById.values()],
    updatedAt: new Date().toISOString(),
  });

  return { ledger: updatedLedger, report };
}
