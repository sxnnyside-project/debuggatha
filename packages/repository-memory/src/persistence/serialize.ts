import { deepFreeze } from "../internal/deep-freeze.js";
import { sortKeysDeep } from "../internal/hash.js";
import { CURRENT_SCHEMA_VERSION, type MemoryStore } from "../types/store.js";

/** Deterministic (sorted keys) and human-readable (2-space JSON) — same format discipline as every other `.debuggatha/*.json` file this repository persists. */
export function serializeMemoryStore(store: MemoryStore): string {
  return `${JSON.stringify(sortKeysDeep(store), null, 2)}\n`;
}

/** Versioned, fails closed on an unrecognized schema rather than guessing at an unknown shape — same discipline as `deserializeLedger`. */
export function deserializeMemoryStore(json: string): MemoryStore {
  const raw = JSON.parse(json) as { schemaVersion?: unknown };
  if (typeof raw.schemaVersion !== "number") {
    throw new Error("Cannot read repository memory: missing or non-numeric schemaVersion.");
  }
  const migrated = migrateMemoryStore(raw, raw.schemaVersion);
  return deepFreeze(migrated as MemoryStore);
}

function migrateMemoryStore(raw: unknown, fromVersion: number): unknown {
  if (fromVersion === CURRENT_SCHEMA_VERSION) return raw;
  throw new Error(
    `Cannot read repository memory: schema version ${fromVersion} is not supported (current: ${CURRENT_SCHEMA_VERSION}). ` +
      "No migration path exists yet for this version gap — this is where a future migration gets added.",
  );
}
