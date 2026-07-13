import { readdirSync, statSync } from "node:fs";
import { join } from "node:path";
import type { ContextIntelligenceResult } from "./types.js";

interface FileStamp {
  path: string;
  mtimeMs: number;
  size: number;
}

interface DirListing {
  dir: string;
  entries: string[];
}

export interface ContextFingerprint {
  /** The `RepositoryContext` this result was built from — reusing its own `generatedAt` covers everything Repository Intelligence already tracks (documentation/criteria/capabilities), without this package re-fingerprinting those files itself. */
  repositoryContextGeneratedAt: string;
  /** Only the files/directories *this* package reads beyond what `RepositoryContext` covers — workflow configs, git refs, license/metadata files. Performance-conscious per Epic 14: targeted, not a full repository walk (unlike Epic 12's Analysis Engine, which genuinely needs one). */
  dirs: DirListing[];
  files: FileStamp[];
}

const TARGETED_FILES = [
  "package.json",
  "composer.json",
  "LICENSE",
  "LICENSE.md",
  "LICENSE.txt",
  "CHANGELOG.md",
  ".release-please-manifest.json",
  "release-please-config.json",
  ".releaserc",
  ".releaserc.json",
  ".releaserc.yml",
  "commitlint.config.js",
  "commitlint.config.ts",
  "commitlint.config.mjs",
  ".commitlintrc.json",
  join(".git", "HEAD"),
  join(".git", "refs", "remotes", "origin", "HEAD"),
];

/** `"."` is the repository root itself — tracked so a new/removed root-level file (e.g. a `CHANGELOG.md` appearing) is caught even though it never existed to `statOrUndefined` before. */
const TARGETED_DIRS = [".", join(".github", "workflows"), join(".git", "refs", "heads")];

function statOrUndefined(path: string): FileStamp | undefined {
  try {
    const stat = statSync(path);
    return { path, mtimeMs: stat.mtimeMs, size: stat.size };
  } catch {
    return undefined;
  }
}

export function buildContextFingerprint(
  rootDir: string,
  repositoryContextGeneratedAt: string,
): ContextFingerprint {
  const files: FileStamp[] = [];
  for (const relative of TARGETED_FILES) {
    const stamp = statOrUndefined(join(rootDir, relative));
    if (stamp) files.push(stamp);
  }

  const dirs: DirListing[] = [];
  for (const relative of TARGETED_DIRS) {
    const full = join(rootDir, relative);
    try {
      dirs.push({ dir: full, entries: [...readdirSync(full)].sort() });
    } catch {
      // directory doesn't exist — absence is itself part of the fingerprint via `hasFingerprintChanged`'s try/catch below
    }
  }

  return { repositoryContextGeneratedAt, dirs, files };
}

export function hasContextFingerprintChanged(
  fingerprint: ContextFingerprint,
  currentRepositoryContextGeneratedAt: string,
): boolean {
  if (fingerprint.repositoryContextGeneratedAt !== currentRepositoryContextGeneratedAt) return true;

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

export interface ContextIntelligenceCacheEntry {
  result: ContextIntelligenceResult;
  fingerprint: ContextFingerprint;
}

/** Injectable, in-memory — same discipline as every prior epic's cache (`RepositoryContextCache`, `AnalysisCache`). */
export interface ContextIntelligenceCache {
  get(rootDir: string): ContextIntelligenceCacheEntry | undefined;
  set(rootDir: string, entry: ContextIntelligenceCacheEntry): void;
}

export function createInMemoryContextCache(): ContextIntelligenceCache {
  const store = new Map<string, ContextIntelligenceCacheEntry>();
  return {
    get: (rootDir) => store.get(rootDir),
    set: (rootDir, entry) => {
      store.set(rootDir, entry);
    },
  };
}
