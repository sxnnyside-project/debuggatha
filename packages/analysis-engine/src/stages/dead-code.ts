import { readFileSync } from "node:fs";
import { join } from "node:path";
import type { DeadCodeFinding, ModuleBoundary, PublicApiSurface } from "../types.js";

/**
 * Dead Code (Epic 12). Deliberately conservative: this stage only ever
 * reports `"exported-but-unused"` (an entrypoint-exported symbol with no
 * matching identifier reference anywhere else in the repo) and
 * `"probably-dead"` (a non-entrypoint exported symbol with no reference
 * outside its own file) — both at `"low"`/`"medium"` confidence, never
 * `"high"`. `"unreachable"`/`"unreferenced"` are real `DeadCodeKind`
 * values this stage's static, name-based approach cannot safely detect
 * without control-flow analysis — reported here only as documented,
 * unimplemented categories (see the package README), not silently
 * guessed at. False positives are worse than false negatives (Epic 12):
 * a symbol referenced anywhere by its bare name, even in an unrelated
 * context, suppresses the finding.
 */
export function detectDeadCode(
  rootDir: string,
  boundaries: ModuleBoundary[],
  publicApis: PublicApiSurface[],
): DeadCodeFinding[] {
  const allFiles = boundaries.flatMap((b) => b.files);
  const fileContents = new Map<string, string>();
  for (const file of allFiles) {
    try {
      fileContents.set(file, readFileSync(join(rootDir, file), "utf8"));
    } catch {
      // unreadable — excluded from the reference search, never assumed dead because of it
    }
  }

  const referenceCount = (name: string, excludeFile: string): number => {
    const pattern = new RegExp(`\\b${escapeRegExp(name)}\\b`, "g");
    let count = 0;
    for (const [file, content] of fileContents) {
      const matches = content.match(pattern);
      if (!matches) continue;
      // Every file's own export declaration line itself matches once — don't count that occurrence.
      count += file === excludeFile ? Math.max(0, matches.length - 1) : matches.length;
    }
    return count;
  };

  const findings: DeadCodeFinding[] = [];

  for (const api of publicApis) {
    for (const symbol of api.exported) {
      if (referenceCount(symbol.name, symbol.file) === 0) {
        findings.push({
          file: symbol.file,
          symbol: symbol.name,
          line: symbol.line,
          kind: "exported-but-unused",
          confidence: "medium",
          rationale: `"${symbol.name}" is exported from ${api.moduleId}'s public entrypoint but no other file in the repository references it by name.`,
        });
      }
    }
    for (const symbol of api.unexpectedlyExposed) {
      if (referenceCount(symbol.name, symbol.file) === 0) {
        findings.push({
          file: symbol.file,
          symbol: symbol.name,
          line: symbol.line,
          kind: "probably-dead",
          confidence: "low",
          rationale: `"${symbol.name}" is exported from a non-entrypoint file in ${api.moduleId} and has no other reference in the repository — likely dead, but this is a name-based heuristic, not certainty.`,
        });
      }
    }
  }

  return findings.sort((a, b) => a.file.localeCompare(b.file) || a.symbol.localeCompare(b.symbol));
}

function escapeRegExp(text: string): string {
  return text.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}
