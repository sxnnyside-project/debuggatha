import type { RepositoryContext } from "../types.js";
import type { Fingerprint } from "./fingerprint.js";

export type { FileStamp, Fingerprint } from "./fingerprint.js";
export { buildFingerprint, hasFingerprintChanged } from "./fingerprint.js";

export interface CacheEntry {
  context: RepositoryContext;
  fingerprint: Fingerprint;
}

/** Injectable so callers control lifetime/scope instead of a hidden global singleton. */
export interface RepositoryContextCache {
  get(rootDir: string): CacheEntry | undefined;
  set(rootDir: string, entry: CacheEntry): void;
}

export function createInMemoryCache(): RepositoryContextCache {
  const store = new Map<string, CacheEntry>();
  return {
    get: (rootDir) => store.get(rootDir),
    set: (rootDir, entry) => {
      store.set(rootDir, entry);
    },
  };
}
