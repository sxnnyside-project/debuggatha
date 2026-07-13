import type { RepositoryContext } from "@debuggatha/repository-intelligence";
import type { ContextIntelligenceCache } from "./cache.js";
import { buildContextFingerprint, hasContextFingerprintChanged } from "./cache.js";
import { buildCapabilityItems } from "./sources/capabilities.js";
import { buildConventionItems } from "./sources/conventions.js";
import { buildDocumentationItems } from "./sources/documentation.js";
import { buildGitContextItems } from "./sources/git.js";
import { buildOwnershipItems } from "./sources/ownership.js";
import { buildProjectMetadataItems } from "./sources/project-metadata.js";
import { detectWorkflow } from "./sources/workflow.js";
import type { ContextIntelligenceResult } from "./types.js";

export interface BuildContextIntelligenceOptions {
  cache?: ContextIntelligenceCache;
}

/**
 * Context Intelligence's one entrypoint (Epic 14): enriches an
 * already-built `RepositoryContext` (Epic 1) with everything that
 * package doesn't cover — Development Workflow, Ownership (via
 * `@debuggatha/analysis-engine`, Epic 12), Project Metadata, and Git
 * Context — while retyping what Repository Intelligence already
 * extracted (`CriteriaProfile`, `DocumentationProfile`, `Capability[]`)
 * into this package's unified, structured-fact `ContextItem` model.
 * Nothing here re-parses a file `RepositoryContext` already read — "do
 * not duplicate Repository Intelligence."
 */
export function buildContextIntelligence(
  rootDir: string,
  repositoryContext: RepositoryContext,
  options: BuildContextIntelligenceOptions = {},
): ContextIntelligenceResult {
  const { cache } = options;

  if (cache) {
    const cached = cache.get(rootDir);
    if (
      cached &&
      !hasContextFingerprintChanged(cached.fingerprint, repositoryContext.generatedAt)
    ) {
      return cached.result;
    }
  }

  const items = [
    ...buildConventionItems(repositoryContext.criteria),
    ...buildDocumentationItems(repositoryContext.documentation),
    ...buildCapabilityItems(repositoryContext.capabilities),
    ...detectWorkflow(rootDir),
    ...buildOwnershipItems(rootDir),
    ...buildProjectMetadataItems(rootDir),
    ...buildGitContextItems(rootDir),
  ];

  const result: ContextIntelligenceResult = {
    rootDir,
    generatedAt: new Date().toISOString(),
    items,
  };

  if (cache) {
    cache.set(rootDir, {
      result,
      fingerprint: buildContextFingerprint(rootDir, repositoryContext.generatedAt),
    });
  }

  return result;
}
