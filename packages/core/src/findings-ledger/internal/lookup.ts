import type { LedgerEntry } from "../types/entry.js";
import type { Ledger } from "../types/ledger.js";

export function getEntry(ledger: Ledger, entryId: string): LedgerEntry | undefined {
  return ledger.entries.find((entry) => entry.id === entryId);
}
