/**
 * The shared repository model every analysis stage enriches (
 * "not isolated utilities... analysis stages composing a shared
 * repository model"). Each stage's output only ever *adds* a field here —
 * nothing downstream mutates or recomputes an earlier stage's data, and
 * nothing here duplicates `RepositoryContext`
 * — `rootDir` is the only
 * overlap, used to key the two independently.
 */

export type ModuleKind = "package" | "workspace" | "feature" | "domain" | "adapter" | "layer";

export interface ModuleBoundary {
  /** Stable id — the module's directory, relative to `rootDir`. */
  id: string;
  /** Absolute directory path. */
  dir: string;
  kind: ModuleKind;
  /** Declared package name when the boundary is backed by a package.json, otherwise undefined. */
  name: string | undefined;
  /** Files (relative to `rootDir`) this boundary owns. */
  files: string[];
}

export type DependencyEdgeKind = "declared" | "import";

export interface DependencyEdge {
  from: string; // ModuleBoundary.id
  to: string; // ModuleBoundary.id
  kind: DependencyEdgeKind;
  /** Evidence: the file(s) and, for `import` edges, the specifier that produced this edge. */
  evidence: { file: string; detail: string }[];
}

export interface DependencyGraph {
  nodes: string[]; // ModuleBoundary.id, sorted
  edges: DependencyEdge[];
}

/** A conventional layer name a module boundary's own name/path was matched against — see `LAYER_VOCABULARY`. */
export type LayerName = (typeof LAYER_ORDER)[number] | "unknown";

/**
 * Outermost (depended-on-by-everything) first. A dependency from a lower
 * index to a higher index is "downward" (allowed); a higher-to-lower
 * dependency is "upward" (a boundary violation) — this is a convention,
 * not a universal truth, which is exactly why `LayerModel.assignments`
 * keeps every module that doesn't match any known vocabulary as
 * `"unknown"` instead of guessing ("do not assume one
 * architectural style").
 */
export const LAYER_ORDER = [
  "domain",
  "application",
  "adapter",
  "infrastructure",
  "presentation",
] as const;

export interface LayerModel {
  assignments: Record<string, LayerName>; // ModuleBoundary.id -> layer
}

export interface LayerViolation {
  from: string;
  to: string;
  fromLayer: LayerName;
  toLayer: LayerName;
  /** Every violation explains both sides — "every finding must explain both sides of the violation". */
  explanation: string;
  evidence: DependencyEdge["evidence"];
}

export interface DependencyCycle {
  /** Module ids participating in the cycle, in cycle order. */
  modules: string[];
  /** The shortest edge sequence closing the cycle. */
  shortestCycle: string[];
  impact: "high" | "medium" | "low";
}

export type ApiStability = "stable" | "unstable" | "unknown";

export interface PublicApiSymbol {
  name: string;
  file: string;
  line: number;
  stability: ApiStability;
}

export interface PublicApiSurface {
  moduleId: string;
  entrypoint: string | undefined;
  exported: PublicApiSymbol[];
  /** Symbols exported from a file *other* than the module's declared entrypoint — "unexpectedly exposed internals". */
  unexpectedlyExposed: PublicApiSymbol[];
}

export type DeadCodeKind = "unreachable" | "unreferenced" | "exported-but-unused" | "probably-dead";

export interface DeadCodeFinding {
  file: string;
  symbol: string;
  line: number;
  kind: DeadCodeKind;
  confidence: "high" | "medium" | "low";
  rationale: string;
}

export type OwnershipSource = "codeowners" | "convention" | "unknown";

export interface OwnershipSignal {
  moduleId: string;
  owners: string[];
  source: OwnershipSource;
}

export interface ChangeImpact {
  changedFiles: string[];
  affectedModules: string[];
  /** Modules not directly changed but depending (transitively) on a changed module. */
  downstreamModules: string[];
  affectedPublicApi: PublicApiSymbol[];
  confidence: "high" | "medium" | "low";
  rationale: string;
}

export interface AnalysisResult {
  rootDir: string;
  generatedAt: string;
  moduleBoundaries: ModuleBoundary[];
  dependencyGraph: DependencyGraph;
  layerModel: LayerModel;
  layerViolations: LayerViolation[];
  cycles: DependencyCycle[];
  publicApi: PublicApiSurface[];
  deadCode: DeadCodeFinding[];
  ownership: OwnershipSignal[];
}
