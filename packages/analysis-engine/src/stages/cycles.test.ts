import { describe, expect, it } from "vitest";
import type { DependencyGraph } from "../types.js";
import { detectCycles } from "./cycles.js";

function edge(from: string, to: string) {
  return { from, to, kind: "import" as const, evidence: [] };
}

describe("detectCycles", () => {
  it("finds no cycles in a DAG", () => {
    const graph: DependencyGraph = {
      nodes: ["a", "b", "c"],
      edges: [edge("a", "b"), edge("b", "c")],
    };
    expect(detectCycles(graph)).toEqual([]);
  });

  it("finds a simple two-node cycle", () => {
    const graph: DependencyGraph = {
      nodes: ["a", "b"],
      edges: [edge("a", "b"), edge("b", "a")],
    };
    const cycles = detectCycles(graph);
    expect(cycles).toHaveLength(1);
    expect(cycles[0]?.modules.sort()).toEqual(["a", "b"]);
    expect(cycles[0]?.impact).toBe("low");
  });

  it("reports a single finding for a component even with multiple internal paths", () => {
    const graph: DependencyGraph = {
      nodes: ["a", "b", "c"],
      edges: [edge("a", "b"), edge("b", "c"), edge("c", "a"), edge("b", "a")],
    };
    const cycles = detectCycles(graph);
    expect(cycles).toHaveLength(1);
    expect(cycles[0]?.modules.sort()).toEqual(["a", "b", "c"]);
    expect(cycles[0]?.impact).toBe("medium");
  });

  it("scales impact with component size", () => {
    const graph: DependencyGraph = {
      nodes: ["a", "b", "c", "d"],
      edges: [edge("a", "b"), edge("b", "c"), edge("c", "d"), edge("d", "a")],
    };
    expect(detectCycles(graph)[0]?.impact).toBe("high");
  });
});
