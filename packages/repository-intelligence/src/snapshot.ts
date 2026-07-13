import { existsSync, readdirSync } from "node:fs";
import { join } from "node:path";
import { buildFingerprint, hasFingerprintChanged } from "./cache/fingerprint.js";
import type { RepositoryContextCache } from "./cache/index.js";
import { deriveCapabilities } from "./capabilities.js";
import { deepFreeze } from "./internal/deep-freeze.js";
import type { DirListing } from "./internal/scan-result.js";
import { scanCriteria } from "./scanners/criteria.js";
import { scanDependencies } from "./scanners/dependencies.js";
import { scanDocumentation } from "./scanners/documentation.js";
import { scanStack } from "./scanners/stack.js";
import { deriveUnderstanding } from "./scanners/understanding.js";
import type { RepositoryContext } from "./types.js";

export interface BuildRepositoryContextOptions {
  /** When provided, an unchanged repository is served from cache instead of rescanned. */
  cache?: RepositoryContextCache;
}

/**
 * The single entrypoint of Repository Intelligence: produces one
 * normalized, immutable `RepositoryContext` by running every scanner
 * once. Everything downstream (Review Engine, MCP, CLI, VS Code — none
 * of which this package depends on) should consume this instead of
 * touching the filesystem itself.
 */
export function buildRepositoryContext(
  rootDir: string,
  options: BuildRepositoryContextOptions = {},
): RepositoryContext {
  const { cache } = options;

  if (cache) {
    const cached = cache.get(rootDir);
    if (cached && !hasFingerprintChanged(cached.fingerprint)) {
      return cached.context;
    }
  }

  const rootEntries = [...readdirSync(rootDir)].sort();
  const rootListing: DirListing = { dir: rootDir, entries: rootEntries };

  const stack = scanStack(rootDir, rootEntries);
  const capabilities = deriveCapabilities(rootDir, rootEntries, stack);
  const dependencies = scanDependencies(rootEntries, stack);
  const documentation = scanDocumentation(rootDir, rootEntries);
  const criteria = scanCriteria(rootDir, rootEntries);
  const understanding = deriveUnderstanding(
    stack.profile,
    dependencies.profile,
    documentation.profile,
    criteria.profile,
  );

  const context: RepositoryContext = deepFreeze({
    rootDir,
    generatedAt: new Date().toISOString(),
    hasGit: existsSync(join(rootDir, ".git")),
    stack: stack.profile,
    capabilities,
    dependencies: dependencies.profile,
    documentation: documentation.profile,
    criteria: criteria.profile,
    understanding,
  });

  if (cache) {
    const filesRead = [
      ...stack.filesRead,
      ...dependencies.filesRead,
      ...documentation.filesRead,
      ...criteria.filesRead,
    ];
    const dirsListed = [
      rootListing,
      ...stack.dirsListed,
      ...documentation.dirsListed,
      ...criteria.dirsListed,
    ];
    cache.set(rootDir, { context, fingerprint: buildFingerprint(dirsListed, filesRead) });
  }

  return context;
}
