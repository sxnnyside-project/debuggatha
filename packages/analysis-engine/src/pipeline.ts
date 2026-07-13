import { type AnalysisCache, buildAnalysisFingerprint, fingerprintChanged } from "./cache.js";
import { detectCycles } from "./stages/cycles.js";
import { detectDeadCode } from "./stages/dead-code.js";
import { buildDependencyGraph } from "./stages/dependency-graph.js";
import { buildLayerModel } from "./stages/layer-model.js";
import { detectLayerViolations } from "./stages/layer-violations.js";
import { inferModuleBoundaries } from "./stages/module-boundaries.js";
import { detectOwnership } from "./stages/ownership.js";
import { buildPublicApiSurface } from "./stages/public-api.js";
import type { AnalysisResult } from "./types.js";

export interface RunAnalysisOptions {
  cache?: AnalysisCache;
}

/**
 * The Analysis Engine's one pipeline (Epic 12): each stage consumes the
 * previous stage's output rather than rebuilding it —
 *
 *   Module Boundaries → Dependency Graph → Layer Model → Layer Violations
 *   → Cycles → Public API Surface → Dead Code → Ownership
 *
 * exactly the order in the epic's diagram (Change Impact is a separate,
 * on-demand entry point — `estimateChangeImpact` — since it needs a set
 * of changed files as input, not something derivable while analyzing the
 * repository at rest). Deterministic and LLM-free throughout — every
 * stage is static analysis over the filesystem, never a model call
 * (Epic 12 "Deterministic First").
 */
export function runAnalysis(rootDir: string, options: RunAnalysisOptions = {}): AnalysisResult {
  const { cache } = options;

  if (cache) {
    const cached = cache.get(rootDir);
    if (cached && !fingerprintChanged(cached.fingerprint)) {
      return cached.result;
    }
  }

  const moduleBoundaries = inferModuleBoundaries(rootDir);
  const dependencyGraph = buildDependencyGraph(rootDir, moduleBoundaries);
  const layerModel = buildLayerModel(moduleBoundaries);
  const layerViolations = detectLayerViolations(dependencyGraph, layerModel);
  const cycles = detectCycles(dependencyGraph);
  const publicApi = moduleBoundaries.map((boundary) => buildPublicApiSurface(rootDir, boundary));
  const deadCode = detectDeadCode(rootDir, moduleBoundaries, publicApi);
  const ownership = detectOwnership(rootDir, moduleBoundaries);

  const result: AnalysisResult = {
    rootDir,
    generatedAt: new Date().toISOString(),
    moduleBoundaries,
    dependencyGraph,
    layerModel,
    layerViolations,
    cycles,
    publicApi,
    deadCode,
    ownership,
  };

  if (cache) {
    cache.set(rootDir, { result, fingerprint: buildAnalysisFingerprint(rootDir) });
  }

  return result;
}
