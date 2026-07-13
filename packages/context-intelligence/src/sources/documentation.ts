import type { DocumentationProfile } from "@debuggatha/repository-intelligence";
import type { DocumentationContextItem } from "../types.js";

/**
 * Documentation context (Epic 14): one item per documentation source
 * `@debuggatha/repository-intelligence`'s Documentation Context (Epic 1)
 * already found — its existence and structural role (`kind`, `title`),
 * never its prose ("extract engineering intent rather than prose").
 * Always `"documented"`: a README/ARCHITECTURE.md/ADR that exists is a
 * fact, not a guess.
 */
export function buildDocumentationItems(
  documentation: DocumentationProfile,
): DocumentationContextItem[] {
  return documentation.sources.map((doc) => ({
    id: `documentation:${doc.file}`,
    category: "documentation",
    confidence: "documented",
    evidence: [{ file: doc.file, detail: `${doc.kind} document, ${doc.wordCount} words` }],
    source: doc.file,
    kind: doc.kind,
    title: doc.title,
  }));
}
