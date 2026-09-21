import { describe, expect, it } from "bun:test";
import type { DependencyGraph, LayerModel } from "../types.js";
import { detectLayerViolations } from "./layer-violations.js";

describe("detectLayerViolations", () => {
  it("flags an upward dependency (adapter depending on domain is fine; domain depending on adapter is not)", () => {
    const layerModel: LayerModel = {
      assignments: { "packages/domain": "domain", "packages/adapter": "adapter" },
    };
    const graph: DependencyGraph = {
      nodes: ["packages/domain", "packages/adapter"],
      edges: [
        {
          from: "packages/domain",
          to: "packages/adapter",
          kind: "import",
          evidence: [{ file: "packages/domain/src/index.ts", detail: 'imports "@x/adapter"' }],
        },
      ],
    };

    const violations = detectLayerViolations(graph, layerModel);
    expect(violations).toHaveLength(1);
    expect(violations[0]?.explanation).toContain("domain");
    expect(violations[0]?.explanation).toContain("adapter");
  });

  it("does not flag a downward dependency (adapter -> domain)", () => {
    const layerModel: LayerModel = {
      assignments: { "packages/domain": "domain", "packages/adapter": "adapter" },
    };
    const graph: DependencyGraph = {
      nodes: ["packages/domain", "packages/adapter"],
      edges: [
        {
          from: "packages/adapter",
          to: "packages/domain",
          kind: "import",
          evidence: [],
        },
      ],
    };
    expect(detectLayerViolations(graph, layerModel)).toEqual([]);
  });

  it("never accuses a module whose layer is unknown", () => {
    const layerModel: LayerModel = { assignments: { a: "unknown", b: "domain" } };
    const graph: DependencyGraph = {
      nodes: ["a", "b"],
      edges: [{ from: "b", to: "a", kind: "import", evidence: [] }],
    };
    expect(detectLayerViolations(graph, layerModel)).toEqual([]);
  });
});
