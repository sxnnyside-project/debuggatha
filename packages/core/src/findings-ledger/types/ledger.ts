import type { LedgerEntry } from "./entry.js";

/** Bumped whenever the persisted shape changes in a way old data can't be read as-is. See `persistence/serialize.ts`. */
export const CURRENT_SCHEMA_VERSION = 1;

/**
 * The historical record of every finding ever produced for one
 * repository (epic §1). A `Ledger` value itself is immutable — every
 * operation in this package (`updateFindingStatus`,
 * `synchronizeReviewResult`) returns a *new* `Ledger`, matching the same
 * discipline `RepositoryContext` and `ReviewPolicy` already established.
 * "Immutable historical record" describes each snapshot in that sequence,
 * not a promise that the Ledger never changes over the repository's life
 * — it accumulates by producing new immutable versions, never by mutating
 * one in place.
 */
export interface Ledger {
  schemaVersion: number;
  repositoryRoot: string;
  entries: LedgerEntry[];
  createdAt: string;
  updatedAt: string;
}

export function createEmptyLedger(repositoryRoot: string): Ledger {
  const now = new Date().toISOString();
  return {
    schemaVersion: CURRENT_SCHEMA_VERSION,
    repositoryRoot,
    entries: [],
    createdAt: now,
    updatedAt: now,
  };
}
