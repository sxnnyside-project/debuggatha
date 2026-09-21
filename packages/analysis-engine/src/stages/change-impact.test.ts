import { describe, expect, it } from "bun:test";
import type { DependencyGraph, ModuleBoundary, PublicApiSurface } from "../types.js";
import { estimateChangeImpact } from "./change-impact.js";

function boundary(id: string, files: string[]): ModuleBoundary {
  return { id, dir: `/repo/${id}`, kind: "package", name: undefined, files };
}

describe("estimateChangeImpact", () => {
  const boundaries = [
    boundary("packages/a", ["packages/a/src/index.ts"]),
    boundary("packages/b", ["packages/b/src/index.ts"]),
    boundary("packages/c", ["packages/c/src/index.ts"]),
  ];
  const graph: DependencyGraph = {
    nodes: ["packages/a", "packages/b", "packages/c"],
    edges: [
      { from: "packages/b", to: "packages/a", kind: "import", evidence: [] },
      { from: "packages/c", to: "packages/b", kind: "import", evidence: [] },
    ],
  };

  it("finds direct and transitive downstream modules", () => {
    const impact = estimateChangeImpact(["packages/a/src/index.ts"], boundaries, graph, []);
    expect(impact.affectedModules).toEqual(["packages/a"]);
    expect(impact.downstreamModules.sort()).toEqual(["packages/b", "packages/c"]);
    expect(impact.confidence).toBe("high");
  });

  it("degrades confidence when a changed file resolves to no known module", () => {
    const impact = estimateChangeImpact(["unknown/file.ts"], boundaries, graph, []);
    expect(impact.affectedModules).toEqual([]);
    expect(impact.confidence).toBe("low");
  });

  it("degrades confidence to medium when the change touches a public API symbol", () => {
    const publicApis: PublicApiSurface[] = [
      {
        moduleId: "packages/a",
        entrypoint: "packages/a/src/index.ts",
        exported: [
          { name: "thing", file: "packages/a/src/index.ts", line: 1, stability: "stable" },
        ],
        unexpectedlyExposed: [],
      },
    ];
    const impact = estimateChangeImpact(["packages/a/src/index.ts"], boundaries, graph, publicApis);
    expect(impact.affectedPublicApi).toHaveLength(1);
    expect(impact.confidence).toBe("medium");
  });
});
