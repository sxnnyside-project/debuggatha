import type { ChangeImpact, DependencyGraph, ModuleBoundary, PublicApiSurface } from "../types.js";

function moduleForFile(boundaries: ModuleBoundary[], file: string): string | undefined {
  return boundaries.find((b) => b.files.includes(file))?.id;
}

/**
 * Change Impact (Epic 12): given the files a pending change touches,
 * estimates blast radius using the Dependency Graph (reverse edges — who
 * depends on the changed modules) and the Public API Surface (does the
 * change touch a symbol another module could be relying on). Never
 * claims certainty — confidence degrades from `"high"` to `"low"` as the
 * estimate leans more on the (heuristic, partial-coverage) Public API
 * Surface and less on directly-observed graph edges.
 */
export function estimateChangeImpact(
  changedFiles: string[],
  boundaries: ModuleBoundary[],
  graph: DependencyGraph,
  publicApis: PublicApiSurface[],
): ChangeImpact {
  const affectedModules = [
    ...new Set(
      changedFiles
        .map((f) => moduleForFile(boundaries, f))
        .filter((m): m is string => m !== undefined),
    ),
  ].sort();

  const reverseAdjacency = new Map<string, string[]>();
  for (const node of graph.nodes) reverseAdjacency.set(node, []);
  for (const edge of graph.edges) reverseAdjacency.get(edge.to)?.push(edge.from);

  const downstream = new Set<string>();
  const queue = [...affectedModules];
  const visited = new Set(affectedModules);
  while (queue.length > 0) {
    const current = queue.shift();
    if (current === undefined) continue;
    for (const dependent of reverseAdjacency.get(current) ?? []) {
      if (visited.has(dependent)) continue;
      visited.add(dependent);
      downstream.add(dependent);
      queue.push(dependent);
    }
  }

  const affectedPublicApi = publicApis
    .filter((api) => affectedModules.includes(api.moduleId))
    .flatMap((api) => api.exported)
    .filter((symbol) => changedFiles.includes(symbol.file));

  let confidence: ChangeImpact["confidence"] = "high";
  let rationale =
    "Every changed file resolved to a known module boundary with a fully-scanned dependency graph.";
  if (affectedModules.length === 0) {
    confidence = "low";
    rationale =
      "None of the changed files resolved to a known module boundary — impact could not be estimated from the Dependency Graph.";
  } else if (affectedPublicApi.length > 0) {
    confidence = "medium";
    rationale = `${affectedPublicApi.length} changed symbol(s) are part of a module's public API surface — downstream impact beyond this repository (external consumers) cannot be measured statically.`;
  }

  return {
    changedFiles: [...changedFiles].sort(),
    affectedModules,
    downstreamModules: [...downstream].sort(),
    affectedPublicApi,
    confidence,
    rationale,
  };
}
