import { deepFreeze } from "../internal/deep-freeze.js";
import { sortKeysDeep } from "../internal/hash.js";
import { CURRENT_SCHEMA_VERSION, type Ledger } from "../types/ledger.js";

/**
 * Deterministic (sorted keys — the same `Ledger` value always serializes
 * to the same bytes, regardless of construction order) and human-readable
 * (2-space indented JSON — inspectable and diffable in a PR, matching
 * every other config file already committed in this monorepo).
 */
export function serializeLedger(ledger: Ledger): string {
  return `${JSON.stringify(sortKeysDeep(ledger), null, 2)}\n`;
}

/**
 * Versioned and resilient to schema evolution (epic §8): every persisted
 * ledger carries `schemaVersion`. `migrateLedger` is where a future
 * version bump adds a real migration step — for now, only the current
 * version is recognized, and anything else fails closed with a clear
 * error rather than guessing at an unknown shape.
 */
export function deserializeLedger(json: string): Ledger {
  const raw = JSON.parse(json) as { schemaVersion?: unknown };
  if (typeof raw.schemaVersion !== "number") {
    throw new Error("Cannot read ledger: missing or non-numeric schemaVersion.");
  }
  const migrated = migrateLedger(raw, raw.schemaVersion);
  return deepFreeze(migrated as Ledger);
}

function migrateLedger(raw: unknown, fromVersion: number): unknown {
  if (fromVersion === CURRENT_SCHEMA_VERSION) return raw;
  throw new Error(
    `Cannot read ledger: schema version ${fromVersion} is not supported (current: ${CURRENT_SCHEMA_VERSION}). ` +
      "No migration path exists yet for this version gap — this is where a future migration gets added.",
  );
}
