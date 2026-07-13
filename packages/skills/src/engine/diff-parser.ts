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
      currentFile = fileHeader[1] === "/dev/null" ? undefined : fileHeader[1];
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
