import { readdirSync, readFileSync, statSync } from "node:fs";
import { join } from "node:path";
import type { DirListing, ScanResult } from "../internal/scan-result.js";
import type { DocumentationKind, DocumentationProfile, DocumentationSource } from "../types.js";

/**
 * Documentation Context.
 *
 * Discovers documentation and summarizes what exists — factually
 * (counts, titles, word counts), never an opinion about its quality or
 * content. Recurses into `docs/` up to MAX_DEPTH so ADRs nested under
 * `docs/adr/` are found, but stays out of build/dependency directories.
 */

const MAX_DEPTH = 4;

const ROOT_DOC_FILES: Record<string, DocumentationKind> = {
  "README.md": "readme",
  "CLAUDE.md": "claude-memory",
  "CONTRIBUTING.md": "contributing",
  "ARCHITECTURE.md": "architecture",
  "ROADMAP.md": "roadmap",
};

const IGNORED_DIRS = new Set([
  "node_modules",
  ".git",
  "dist",
  ".turbo",
  "target",
  "build",
  "legacy",
]);

function classify(fileName: string, relativeDir: string): DocumentationKind {
  const lower = fileName.toLowerCase();
  if (relativeDir.toLowerCase().includes("adr") || /^\d{4}[-_]/.test(lower)) return "adr";
  if (/prd/.test(lower)) return "prd";
  if (/roadmap/.test(lower)) return "roadmap";
  if (/architecture/.test(lower)) return "architecture";
  if (/contributing/.test(lower)) return "contributing";
  if (/^readme/.test(lower)) return "readme";
  if (lower === "claude.md") return "claude-memory";
  return "other";
}

function extractTitle(content: string): string | undefined {
  return content.match(/^#\s+(.+)$/m)?.[1]?.trim();
}

function countWords(content: string): number {
  const trimmed = content.trim();
  return trimmed.length === 0 ? 0 : trimmed.split(/\s+/).length;
}

function buildSummary(sources: DocumentationSource[]): string {
  if (sources.length === 0) return "No documentation sources found.";
  const counts = new Map<string, number>();
  for (const source of sources) counts.set(source.kind, (counts.get(source.kind) ?? 0) + 1);
  const parts = [...counts.entries()].map(
    ([kind, count]) => `${count} ${kind}${count > 1 ? "s" : ""}`,
  );
  return `${sources.length} documentation source${sources.length > 1 ? "s" : ""} found: ${parts.join(", ")}.`;
}

export function scanDocumentation(
  rootDir: string,
  rootEntries: readonly string[],
): ScanResult<DocumentationProfile> {
  const entries = new Set(rootEntries);
  const filesRead: string[] = [];
  const dirsListed: DirListing[] = [];
  const sources: DocumentationSource[] = [];

  for (const [file, kind] of Object.entries(ROOT_DOC_FILES)) {
    if (entries.has(file)) {
      const filePath = join(rootDir, file);
      filesRead.push(filePath);
      const content = readFileSync(filePath, "utf8");
      sources.push({ file, kind, title: extractTitle(content), wordCount: countWords(content) });
    }
  }

  if (entries.has("docs")) {
    walk(join(rootDir, "docs"), "docs", 0);
  }

  function walk(dir: string, relativeDir: string, depth: number): void {
    if (depth > MAX_DEPTH) return;
    const dirEntries = [...readdirSync(dir)].sort();
    dirsListed.push({ dir, entries: dirEntries });

    for (const entry of dirEntries) {
      if (IGNORED_DIRS.has(entry)) continue;
      const fullPath = join(dir, entry);
      const stats = statSync(fullPath);

      if (stats.isDirectory()) {
        walk(fullPath, `${relativeDir}/${entry}`, depth + 1);
      } else if (entry.toLowerCase().endsWith(".md")) {
        filesRead.push(fullPath);
        const content = readFileSync(fullPath, "utf8");
        const relativeFile = `${relativeDir}/${entry}`;
        sources.push({
          file: relativeFile,
          kind: classify(entry, relativeDir),
          title: extractTitle(content),
          wordCount: countWords(content),
        });
      }
    }
  }

  const profile: DocumentationProfile = {
    sources: sources.sort((a, b) => a.file.localeCompare(b.file)),
    summary: buildSummary(sources),
  };

  return { profile, filesRead, dirsListed };
}
