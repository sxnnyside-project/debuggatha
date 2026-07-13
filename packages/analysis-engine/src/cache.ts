import { readdirSync, statSync } from "node:fs";
import { join } from "node:path";
import { IGNORED_DIRS } from "./internal/fs-walk.js";
import type { AnalysisResult } from "./types.js";

export interface AnalysisFileStamp {
  path: string;
  mtimeMs: number;
  size: number;
}

export interface AnalysisDirListing {
  dir: string;
  entries: string[];
}

export interface AnalysisFingerprint {
  dirs: AnalysisDirListing[];
  files: AnalysisFileStamp[];
}

/**
 * Same shape/strategy as `@debuggatha/repository-intelligence`'s own
 * fingerprint cache — a separate, independent implementation (not a
 * shared dependency: Epic 12 must not duplicate Repository Intelligence
 * *responsibilities*, but stat-based staleness detection isn't one of
 * those; it's how any repo-wide cache invalidates). Records both every
 * directory's listing (catches a file being added/removed, which no
 * per-file stat would ever notice) and every file's mtime/size (catches
 * a tracked file's content changing) — mirroring exactly why Epic 1's
 * own fingerprint tracks both, not just one.
 */
export function buildAnalysisFingerprint(rootDir: string): AnalysisFingerprint {
  const files: AnalysisFileStamp[] = [];
  const dirs: AnalysisDirListing[] = [];

  const walk = (dir: string) => {
    let entries: string[];
    try {
      entries = [...readdirSync(dir)].sort();
    } catch {
      return;
    }
    dirs.push({ dir, entries });
    for (const entry of entries) {
      if (IGNORED_DIRS.has(entry) || entry.startsWith(".")) continue;
      const full = join(dir, entry);
      let stat: ReturnType<typeof statSync>;
      try {
        stat = statSync(full);
      } catch {
        continue;
      }
      if (stat.isDirectory()) walk(full);
      else files.push({ path: full, mtimeMs: stat.mtimeMs, size: stat.size });
    }
  };
  walk(rootDir);

  return { dirs, files };
}

export function fingerprintChanged(fingerprint: AnalysisFingerprint): boolean {
  for (const listing of fingerprint.dirs) {
    let currentEntries: string[];
    try {
      currentEntries = [...readdirSync(listing.dir)].sort();
    } catch {
      return true;
    }
    if (currentEntries.length !== listing.entries.length) return true;
    for (let i = 0; i < currentEntries.length; i++) {
      if (currentEntries[i] !== listing.entries[i]) return true;
    }
  }

  for (const file of fingerprint.files) {
    try {
      const stat = statSync(file.path);
      if (stat.mtimeMs !== file.mtimeMs || stat.size !== file.size) return true;
    } catch {
      return true;
    }
  }

  return false;
}

export interface AnalysisCacheEntry {
  result: AnalysisResult;
  fingerprint: AnalysisFingerprint;
}

/** Injectable, in-memory — same discipline as `RepositoryContextCache` (Epic 1): no hidden global singleton, caller controls lifetime/scope. */
export interface AnalysisCache {
  get(rootDir: string): AnalysisCacheEntry | undefined;
  set(rootDir: string, entry: AnalysisCacheEntry): void;
}

export function createInMemoryAnalysisCache(): AnalysisCache {
  const store = new Map<string, AnalysisCacheEntry>();
  return {
    get: (rootDir) => store.get(rootDir),
    set: (rootDir, entry) => {
      store.set(rootDir, entry);
    },
  };
}
