export {
  type AnalysisCache,
  type AnalysisCacheEntry,
  type AnalysisFingerprint,
  buildAnalysisFingerprint,
  createInMemoryAnalysisCache,
  fingerprintChanged,
} from "./cache.js";
export { type RunAnalysisOptions, runAnalysis } from "./pipeline.js";
export { estimateChangeImpact } from "./stages/change-impact.js";
export { detectCycles } from "./stages/cycles.js";
export { detectDeadCode } from "./stages/dead-code.js";
export { buildDependencyGraph } from "./stages/dependency-graph.js";
export { buildLayerModel } from "./stages/layer-model.js";
export { detectLayerViolations } from "./stages/layer-violations.js";
export { inferModuleBoundaries } from "./stages/module-boundaries.js";
export { detectOwnership } from "./stages/ownership.js";
export { buildPublicApiSurface } from "./stages/public-api.js";
export * from "./types.js";
