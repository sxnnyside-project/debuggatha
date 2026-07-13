import { readdirSync, readFileSync, statSync } from "node:fs";
import { join, relative } from "node:path";
import type { SourceUnit } from "./run.js";

const IGNORED_DIRS = new Set([
  "node_modules",
  ".git",
  "dist",
  "build",
  ".turbo",
  "coverage",
  ".next",
  ".venv",
]);

const SOURCE_EXTENSIONS = /\.(ts|tsx|js|jsx|mjs|cjs|dart|rs|go|py|kt|php|cs)$/;

/** Reads explicit files (File Review) as whole-file `SourceUnit`s — `lineNumbers` is `undefined` since content already is the whole file. */
export function readSourceUnits(paths: string[]): SourceUnit[] {
  const units: SourceUnit[] = [];
  for (const path of paths) {
    try {
      const content = readFileSync(path, "utf8");
      units.push({ file: path, content, lineNumbers: undefined });
    } catch {
      // unreadable path (deleted, permissions, binary) — skip, don't fail the whole review
    }
  }
  return units;
}

/** Walks a directory tree for Architecture Review, skipping build output/dependency directories. */
export function walkSourceUnits(rootDir: string): SourceUnit[] {
  const units: SourceUnit[] = [];

  const walk = (dir: string) => {
    let entries: string[];
    try {
      entries = readdirSync(dir);
    } catch {
      return;
    }
    for (const entry of entries) {
      if (IGNORED_DIRS.has(entry) || entry.startsWith(".")) continue;
      const full = join(dir, entry);
      let stat: ReturnType<typeof statSync>;
      try {
        stat = statSync(full);
      } catch {
        continue;
      }
      if (stat.isDirectory()) {
        walk(full);
      } else if (SOURCE_EXTENSIONS.test(entry)) {
        try {
          const content = readFileSync(full, "utf8");
          units.push({ file: relative(rootDir, full), content, lineNumbers: undefined });
        } catch {
          // unreadable file — skip
        }
      }
    }
  };

  walk(rootDir);
  return units;
}
