import {
  type DependencyGraph,
  LAYER_ORDER,
  type LayerModel,
  type LayerViolation,
} from "../types.js";

/**
 * Layer Violations: compares the Dependency Graph against the
 * Layer Model — consumes both rather than re-deriving either. Only edges
 * where *both* ends resolved to a known layer (never `"unknown"`) are
 * checked; a module the Layer Model couldn't confidently place is never
 * used to accuse another module of a violation. One violation per
 * `(from, to)` pair even when both a `declared` and an `import` edge
 * exist for it — same underlying problem, evidence merged rather than
 * reported twice.
 */
export function detectLayerViolations(
  graph: DependencyGraph,
  layerModel: LayerModel,
): LayerViolation[] {
  const byPair = new Map<string, LayerViolation>();

  for (const edge of graph.edges) {
    const fromLayer = layerModel.assignments[edge.from] ?? "unknown";
    const toLayer = layerModel.assignments[edge.to] ?? "unknown";
    if (fromLayer === "unknown" || toLayer === "unknown") continue;
    if (fromLayer === toLayer) continue;

    const fromIndex = LAYER_ORDER.indexOf(fromLayer as (typeof LAYER_ORDER)[number]);
    const toIndex = LAYER_ORDER.indexOf(toLayer as (typeof LAYER_ORDER)[number]);
    // Outer layers (higher index) may depend on inner ones (lower index) —
    // e.g. an "adapter" depending on "domain" — never the reverse.
    if (fromIndex >= toIndex) continue;

    const key = `${edge.from}->${edge.to}`;
    let violation = byPair.get(key);
    if (!violation) {
      violation = {
        from: edge.from,
        to: edge.to,
        fromLayer,
        toLayer,
        explanation: `"${edge.from}" (layer "${fromLayer}") depends on "${edge.to}" (layer "${toLayer}"), but "${toLayer}" is conventionally outer relative to "${fromLayer}" — this is an upward dependency against the assumed layer order (${LAYER_ORDER.join(" -> ")}).`,
        evidence: [],
      };
      byPair.set(key, violation);
    }
    violation.evidence.push(...edge.evidence);
  }

  return [...byPair.values()].sort(
    (a, b) => a.from.localeCompare(b.from) || a.to.localeCompare(b.to),
  );
}
