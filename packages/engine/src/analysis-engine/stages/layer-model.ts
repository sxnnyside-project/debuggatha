import { LAYER_ORDER, type LayerModel, type LayerName, type ModuleBoundary } from "../types.js";

/**
 * Vocabulary a module's own id/name is checked against, case-insensitive,
 * to assign it a conventional layer. Deliberately narrow and additive —
 * a module matching none of these stays `"unknown"` rather than being
 * forced into a guessed layer ("do not assume one architectural
 * style").
 */
const LAYER_VOCABULARY: Record<(typeof LAYER_ORDER)[number], string[]> = {
  domain: ["domain", "domains", "core", "entities", "model", "models"],
  application: ["application", "app", "use-cases", "usecases", "services"],
  adapter: ["adapter", "adapters", "gateways", "repositories"],
  infrastructure: ["infrastructure", "infra", "persistence", "data"],
  presentation: ["presentation", "ui", "web", "views", "components", "pages"],
};

function matchLayer(text: string): LayerName {
  const lower = text.toLowerCase();
  for (const layer of LAYER_ORDER) {
    if (
      LAYER_VOCABULARY[layer].some(
        (word) => lower === word || lower.includes(`/${word}`) || lower.endsWith(word),
      )
    ) {
      return layer;
    }
  }
  return "unknown";
}

/** Layer Model: assigns each Module Boundary a conventional layer name, purely from its id/name — never from its dependencies (that would be circular with Layer Violations, which consumes this). */
export function buildLayerModel(boundaries: ModuleBoundary[]): LayerModel {
  const assignments: Record<string, LayerName> = {};
  for (const boundary of boundaries) {
    assignments[boundary.id] = matchLayer(boundary.name ?? boundary.id);
  }
  return { assignments };
}
