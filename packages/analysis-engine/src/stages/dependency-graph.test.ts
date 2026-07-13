import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import { buildDependencyGraph } from "./dependency-graph.js";
import { inferModuleBoundaries } from "./module-boundaries.js";

let dir: string | undefined;

afterEach(() => {
  if (dir) rmSync(dir, { recursive: true, force: true });
  dir = undefined;
});

function mktemp(): string {
  return mkdtempSync(join(tmpdir(), "debuggatha-depgraph-"));
}

describe("buildDependencyGraph", () => {
  it("builds a declared edge from a package's own package.json", () => {
    dir = mktemp();
    mkdirSync(join(dir, "packages", "a", "src"), { recursive: true });
    mkdirSync(join(dir, "packages", "b", "src"), { recursive: true });
    writeFileSync(
      join(dir, "packages", "a", "package.json"),
      JSON.stringify({ name: "@x/a", dependencies: { "@x/b": "workspace:*" } }),
    );
    writeFileSync(join(dir, "packages", "b", "package.json"), JSON.stringify({ name: "@x/b" }));

    const boundaries = inferModuleBoundaries(dir);
    const graph = buildDependencyGraph(dir, boundaries);

    expect(graph.nodes).toEqual(["packages/a", "packages/b"]);
    expect(graph.edges).toHaveLength(1);
    expect(graph.edges[0]).toMatchObject({
      from: "packages/a",
      to: "packages/b",
      kind: "declared",
    });
  });

  it("builds an import edge from a source file importing another package's name", () => {
    dir = mktemp();
    mkdirSync(join(dir, "packages", "a", "src"), { recursive: true });
    mkdirSync(join(dir, "packages", "b", "src"), { recursive: true });
    writeFileSync(join(dir, "packages", "a", "package.json"), JSON.stringify({ name: "@x/a" }));
    writeFileSync(join(dir, "packages", "b", "package.json"), JSON.stringify({ name: "@x/b" }));
    writeFileSync(join(dir, "packages", "a", "src", "index.ts"), 'import { thing } from "@x/b";\n');

    const boundaries = inferModuleBoundaries(dir);
    const graph = buildDependencyGraph(dir, boundaries);

    expect(graph.edges).toContainEqual(
      expect.objectContaining({ from: "packages/a", to: "packages/b", kind: "import" }),
    );
  });

  it("does not create a self-edge for intra-module imports", () => {
    dir = mktemp();
    mkdirSync(join(dir, "packages", "a", "src"), { recursive: true });
    writeFileSync(join(dir, "packages", "a", "package.json"), JSON.stringify({ name: "@x/a" }));
    writeFileSync(join(dir, "packages", "a", "src", "index.ts"), 'import "./helper.js";\n');
    writeFileSync(join(dir, "packages", "a", "src", "helper.ts"), "export const h = 1;\n");

    const boundaries = inferModuleBoundaries(dir);
    const graph = buildDependencyGraph(dir, boundaries);
    expect(graph.edges).toEqual([]);
  });

  it("deduplicates repeated import edges into one, accumulating evidence", () => {
    dir = mktemp();
    mkdirSync(join(dir, "packages", "a", "src"), { recursive: true });
    mkdirSync(join(dir, "packages", "b", "src"), { recursive: true });
    writeFileSync(join(dir, "packages", "a", "package.json"), JSON.stringify({ name: "@x/a" }));
    writeFileSync(join(dir, "packages", "b", "package.json"), JSON.stringify({ name: "@x/b" }));
    writeFileSync(join(dir, "packages", "a", "src", "one.ts"), 'import "@x/b";\n');
    writeFileSync(join(dir, "packages", "a", "src", "two.ts"), 'import "@x/b";\n');

    const boundaries = inferModuleBoundaries(dir);
    const graph = buildDependencyGraph(dir, boundaries);
    const importEdges = graph.edges.filter((e) => e.kind === "import");
    expect(importEdges).toHaveLength(1);
    expect(importEdges[0]?.evidence).toHaveLength(2);
  });
});
