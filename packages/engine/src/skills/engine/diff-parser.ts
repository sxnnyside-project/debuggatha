import { isIgnoredPath } from "../../scan-scope/index.js";
import type { SourceUnit } from "./run.js";

/**
 * Turns a unified diff into one `SourceUnit` per changed file, containing
 * only the *added* lines (never the file's untouched context) — this is
 * what keeps Diff Review from re-reviewing the whole repository (see the
 * epic's "Diff Review... avoid re-reviewing the entire repository").
 * Removed/context lines are read for hunk bookkeeping only, never handed
 * to a detector.
 */
export function parseUnifiedDiff(diff: string): SourceUnit[] {
  const units: SourceUnit[] = [];
  let currentFile: string | undefined;
  let addedLines: string[] = [];
  let addedLineNumbers: number[] = [];
  let newLine = 0;

  const flush = () => {
    if (currentFile && addedLines.length > 0) {
      units.push({
        file: currentFile,
        content: addedLines.join("\n"),
        lineNumbers: addedLineNumbers,
      });
    }
    addedLines = [];
    addedLineNumbers = [];
  };

  const lines = diff.split("\n");
  for (const line of lines) {
    const fileHeader = /^\+\+\+ (?:b\/)?(.+)$/.exec(line);
    if (fileHeader) {
      flush();
      const path = fileHeader[1];
      // A path the project does not own (dependencies, build output, generated) is never reviewed.
      currentFile =
        path === undefined || path === "/dev/null" || isIgnoredPath(undefined, path)
          ? undefined
          : path;
      continue;
    }

    const hunkHeader = /^@@ -\d+(?:,\d+)? \+(\d+)(?:,\d+)? @@/.exec(line);
    if (hunkHeader) {
      newLine = Number.parseInt(hunkHeader[1] ?? "1", 10);
      continue;
    }

    if (!currentFile) continue;

    if (line.startsWith("+") && !line.startsWith("+++")) {
      addedLines.push(line.slice(1));
      addedLineNumbers.push(newLine);
      newLine += 1;
    } else if (line.startsWith("-") && !line.startsWith("---")) {
      // removed line — doesn't advance newLine, not scanned
    } else if (!line.startsWith("\\")) {
      newLine += 1;
    }
  }
  flush();

  return units;
}

/**
 * The lines a diff adds, per file: what "your change" means when a review should fail only on
 * what a pull request introduced, not on what was already there.
 */
export function addedLinesByFile(diff: string): Map<string, Set<number>> {
  const lines = new Map<string, Set<number>>();
  for (const unit of parseUnifiedDiff(diff)) {
    const set = lines.get(unit.file) ?? new Set<number>();
    unit.lineNumbers?.forEach((n) => {
      set.add(n);
    });
    lines.set(unit.file, set);
  }
  return lines;
}
