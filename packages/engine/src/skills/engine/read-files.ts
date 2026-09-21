import { readFileSync } from "node:fs";
import { join } from "node:path";
import { isGeneratedContent, listScanFiles } from "../../scan-scope/index.js";
import type { SourceUnit } from "./run.js";

const SOURCE_EXTENSIONS = /\.(ts|tsx|js|jsx|mjs|cjs|dart|rs|go|py|kt|kts|php|cs|java|vue|svelte)$/;

/**
 * Reads explicit files (File Review) as whole-file `SourceUnit`s — `lineNumbers` is `undefined` since content already is the whole file.
 * A file the user names is reviewed wherever it lives, but generated or minified content is still skipped.
 */
export function readSourceUnits(paths: string[]): SourceUnit[] {
  const units: SourceUnit[] = [];
  for (const path of paths) {
    try {
      const content = readFileSync(path, "utf8");
      if (isGeneratedContent(content)) continue;
      units.push({ file: path, content, lineNumbers: undefined });
    } catch {
      // unreadable path (deleted, permissions, binary) — skip, don't fail the whole review
    }
  }
  return units;
}

/**
 * Reads every file the project owns for Architecture Review: what git tracks
 * or would track (so `.gitignore` decides), minus dependencies, build output,
 * and generated or minified files.
 */
export function walkSourceUnits(rootDir: string): SourceUnit[] {
  const units: SourceUnit[] = [];
  for (const file of listScanFiles(rootDir, SOURCE_EXTENSIONS)) {
    try {
      const content = readFileSync(join(rootDir, file), "utf8");
      if (isGeneratedContent(content)) continue;
      units.push({ file, content, lineNumbers: undefined });
    } catch {
      // unreadable file — skip
    }
  }
  return units;
}
