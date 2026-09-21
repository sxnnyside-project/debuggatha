import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { createEmptyMemoryStore, type MemoryStore } from "../types/store.js";
import { deserializeMemoryStore, serializeMemoryStore } from "./serialize.js";

/** `.debuggatha/memory.json` — inside the same directory the Findings Ledger already uses. */
export function memoryFilePath(repositoryRoot: string): string {
  return join(repositoryRoot, ".debuggatha", "memory.json");
}

/** A repository with no memory yet is not an error — it has an empty store, same as `loadLedger`. */
export function loadMemoryStore(repositoryRoot: string): MemoryStore {
  const path = memoryFilePath(repositoryRoot);
  if (!existsSync(path)) return createEmptyMemoryStore(repositoryRoot);
  return deserializeMemoryStore(readFileSync(path, "utf8"));
}

export function saveMemoryStore(store: MemoryStore): void {
  const path = memoryFilePath(store.repositoryRoot);
  mkdirSync(dirname(path), { recursive: true });
  writeFileSync(path, serializeMemoryStore(store), "utf8");
}
