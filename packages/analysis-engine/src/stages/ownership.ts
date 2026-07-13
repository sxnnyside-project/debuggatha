import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";
import type { ModuleBoundary, OwnershipSignal } from "../types.js";

interface CodeownersRule {
  pattern: string;
  owners: string[];
}

function findCodeownersPath(rootDir: string): string | undefined {
  const candidates = ["CODEOWNERS", ".github/CODEOWNERS", "docs/CODEOWNERS"];
  for (const candidate of candidates) {
    const full = join(rootDir, candidate);
    if (existsSync(full)) return full;
  }
  return undefined;
}

function parseCodeowners(content: string): CodeownersRule[] {
  const rules: CodeownersRule[] = [];
  for (const rawLine of content.split("\n")) {
    const line = rawLine.trim();
    if (!line || line.startsWith("#")) continue;
    const [pattern, ...owners] = line.split(/\s+/);
    if (!pattern || owners.length === 0) continue;
    rules.push({ pattern, owners });
  }
  return rules;
}

/** Real (if simplified) CODEOWNERS glob matching: `*` is any path segment content, `**` any depth, patterns anchored at repo root unless prefixed with `/`. Last matching rule wins — real CODEOWNERS semantics. */
function patternMatches(pattern: string, path: string): boolean {
  const normalized = pattern.startsWith("/") ? pattern.slice(1) : pattern;
  const withoutTrailingSlash = normalized.endsWith("/") ? normalized.slice(0, -1) : normalized;
  const regexSource = withoutTrailingSlash
    .split("/")
    .map((segment) =>
      segment === "**"
        ? ".*"
        : segment.replace(/[.+^${}()|[\]\\]/g, "\\$&").replace(/\*/g, "[^/]*"),
    )
    .join("/");
  const regex = new RegExp(`^${regexSource}(?:/.*)?$`);
  return regex.test(path);
}

/**
 * Ownership Detection (Epic 12). `CODEOWNERS` (checked at the three
 * conventional locations) is the only source treated as confident —
 * real repository convention, not a guess. When no `CODEOWNERS` entry
 * matches a module, ownership is reported `"unknown"` rather than
 * inferred from, e.g., commit history — "if ownership cannot be
 * inferred confidently, report unknown instead of guessing".
 */
export function detectOwnership(rootDir: string, boundaries: ModuleBoundary[]): OwnershipSignal[] {
  const codeownersPath = findCodeownersPath(rootDir);
  const rules = codeownersPath ? parseCodeowners(readFileSync(codeownersPath, "utf8")) : [];

  return boundaries.map((boundary) => {
    let owners: string[] = [];
    for (const rule of rules) {
      if (patternMatches(rule.pattern, boundary.id)) owners = rule.owners; // last match wins
    }
    return {
      moduleId: boundary.id,
      owners,
      source: owners.length > 0 ? "codeowners" : "unknown",
    };
  });
}
