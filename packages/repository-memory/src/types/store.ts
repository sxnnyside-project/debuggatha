import type { MemoryItem } from "./item.js";

/** Bumped whenever the persisted shape changes in a way old data can't be read as-is — see `persistence/serialize.ts`. */
export const CURRENT_SCHEMA_VERSION = 1;

/**
 * The persisted store of everything Repository Memory remembers for one
 * repository. Immutable, same discipline as `Ledger`
 * (`@debuggatha/findings-ledger`) — every operation in this package
 * returns a *new* `MemoryStore`.
 */
export interface MemoryStore {
  schemaVersion: number;
  repositoryRoot: string;
  items: MemoryItem[];
  createdAt: string;
  updatedAt: string;
}

export function createEmptyMemoryStore(repositoryRoot: string): MemoryStore {
  const now = new Date().toISOString();
  return {
    schemaVersion: CURRENT_SCHEMA_VERSION,
    repositoryRoot,
    items: [],
    createdAt: now,
    updatedAt: now,
  };
}
