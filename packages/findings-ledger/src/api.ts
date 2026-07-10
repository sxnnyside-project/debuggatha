import type { Severity } from "@debuggatha/review-engine";
import { deepFreeze } from "./internal/deep-freeze.js";
import { getEntry } from "./internal/lookup.js";
import type { LedgerEntry } from "./types/entry.js";
import { transitionFinding } from "./types/entry.js";
import type { HistoryEvent, TransitionOrigin } from "./types/history.js";
import type { Ledger } from "./types/ledger.js";
import type { FindingLifecycleStatus } from "./types/lifecycle.js";

export { getEntry } from "./internal/lookup.js";

export interface LedgerEntryFilter {
  status?: FindingLifecycleStatus[];
  severity?: Severity[];
  category?: string[];
  file?: string;
}

/** Querying findings (epic §9) — the read side of the public API. */
export function listEntries(ledger: Ledger, filter: LedgerEntryFilter = {}): LedgerEntry[] {
  return ledger.entries.filter((entry) => {
    if (filter.status && !filter.status.includes(entry.status)) return false;
    if (filter.severity && !filter.severity.includes(entry.latestFinding.severity)) return false;
    if (filter.category && !filter.category.includes(entry.latestFinding.category)) return false;
    if (filter.file !== undefined && entry.fingerprint.file !== filter.file) return false;
    return true;
  });
}

/** Reading history (epic §9). */
export function getHistory(ledger: Ledger, entryId: string): HistoryEvent[] {
  const entry = getEntry(ledger, entryId);
  if (!entry) {
    throw new Error(`No ledger entry with id "${entryId}".`);
  }
  return entry.history;
}

/**
 * Updating lifecycle (epic §9) — the *only* sanctioned way to change a
 * finding's status. "Adapters should never manipulate persistence
 * directly": every future adapter (CLI, MCP, VS Code) calls this instead
 * of touching `ledger.entries` itself.
 */
export function updateFindingStatus(
  ledger: Ledger,
  entryId: string,
  to: FindingLifecycleStatus,
  origin: TransitionOrigin,
  comment?: string,
): Ledger {
  const entry = getEntry(ledger, entryId);
  if (!entry) {
    throw new Error(`No ledger entry with id "${entryId}".`);
  }
  const updated = transitionFinding(entry, to, origin, comment);
  return deepFreeze({
    ...ledger,
    entries: ledger.entries.map((existing) => (existing.id === entryId ? updated : existing)),
    updatedAt: new Date().toISOString(),
  });
}
