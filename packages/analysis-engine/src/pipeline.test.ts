import { mkdirSync, mkdtempSync, rmSync, utimesSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import { createInMemoryAnalysisCache } from "./cache.js";
import { runAnalysis } from "./pipeline.js";

let dir: string | undefined;

afterEach(() => {
  if (dir) rmSync(dir, { recursive: true, force: true });
  dir = undefined;
});

function mktemp(): string {
  return mkdtempSync(join(tmpdir(), "debuggatha-pipeline-"));
}

function fixtureRepo(): string {
  const root = mktemp();
  mkdirSync(join(root, "packages", "domain-users", "src"), { recursive: true });
  mkdirSync(join(root, "packages", "adapters-db", "src"), { recursive: true });
  writeFileSync(
    join(root, "packages", "domain-users", "package.json"),
    JSON.stringify({ name: "@x/domain-users", dependencies: { "@x/adapters-db": "workspace:*" } }),
  );
  writeFileSync(
    join(root, "packages", "adapters-db", "package.json"),
    JSON.stringify({ name: "@x/adapters-db" }),
  );
  writeFileSync(
    join(root, "packages", "domain-users", "src", "index.ts"),
    'import { Db } from "@x/adapters-db";\nexport function getUser() {}\n',
  );
  writeFileSync(join(root, "packages", "adapters-db", "src", "index.ts"), "export class Db {}\n");
  return root;
}

describe("runAnalysis", () => {
  it("composes every stage into one AnalysisResult", () => {
    dir = fixtureRepo();
    const result = runAnalysis(dir);

    expect(result.moduleBoundaries.map((b) => b.id)).toEqual([
      "packages/adapters-db",
      "packages/domain-users",
    ]);
    expect(result.dependencyGraph.edges.length).toBeGreaterThan(0);
    expect(result.layerModel.assignments["packages/domain-users"]).toBe("domain");
    expect(result.layerModel.assignments["packages/adapters-db"]).toBe("adapter");
    // domain depending on adapter is an upward violation under the assumed layer order.
    expect(result.layerViolations).toHaveLength(1);
    expect(result.cycles).toEqual([]);
    expect(
      result.publicApi
        .find((a) => a.moduleId === "packages/adapters-db")
        ?.exported.map((s) => s.name),
    ).toEqual(["Db"]);
    expect(result.ownership.every((o) => o.source === "unknown")).toBe(true);
  });

  it("serves a cached result when the repository fingerprint hasn't changed", () => {
    dir = fixtureRepo();
    const cache = createInMemoryAnalysisCache();
    const first = runAnalysis(dir, { cache });
    const second = runAnalysis(dir, { cache });
    expect(second).toBe(first);
  });

  it("invalidates the cache when a file changes", () => {
    dir = fixtureRepo();
    const cache = createInMemoryAnalysisCache();
    const first = runAnalysis(dir, { cache });

    const newFile = join(dir, "packages", "domain-users", "src", "extra.ts");
    writeFileSync(newFile, "export function extra() {}\n");
    const future = new Date(Date.now() + 5000);
    utimesSync(newFile, future, future);

    const second = runAnalysis(dir, { cache });
    expect(second).not.toBe(first);
    expect(second.moduleBoundaries.find((b) => b.id === "packages/domain-users")?.files).toContain(
      "packages/domain-users/src/extra.ts",
    );
  });
});
