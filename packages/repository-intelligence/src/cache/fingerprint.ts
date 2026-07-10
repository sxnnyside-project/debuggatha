import { readdirSync, statSync } from "node:fs";
import type { DirListing } from "../internal/scan-result.js";

/**
 * What a scan actually read: every directory it listed and every file it
 * opened. Comparing this on the next call is cheap (stat calls, not
 * re-reading/re-parsing file contents) while still catching "correctness"
 * cases a naive mtime-only check would miss — a new manifest appearing in
 * a previously-empty directory changes that directory's listing even
 * though no *tracked* file's mtime changed.
 */

export interface FileStamp {
  path: string;
  mtimeMs: number;
  size: number;
}

export interface Fingerprint {
  dirs: DirListing[];
  files: FileStamp[];
}

function dedupeDirs(dirs: DirListing[]): DirListing[] {
  const byDir = new Map<string, DirListing>();
  for (const listing of dirs) byDir.set(listing.dir, listing);
  return [...byDir.values()].sort((a, b) => a.dir.localeCompare(b.dir));
}

export function buildFingerprint(dirs: DirListing[], filePaths: string[]): Fingerprint {
  const files = [...new Set(filePaths)].sort().map((path) => {
    const stats = statSync(path);
    return { path, mtimeMs: stats.mtimeMs, size: stats.size };
  });

  return { dirs: dedupeDirs(dirs), files };
}

export function hasFingerprintChanged(fingerprint: Fingerprint): boolean {
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
      const stats = statSync(file.path);
      if (stats.mtimeMs !== file.mtimeMs || stats.size !== file.size) return true;
    } catch {
      return true;
    }
  }

  return false;
}
