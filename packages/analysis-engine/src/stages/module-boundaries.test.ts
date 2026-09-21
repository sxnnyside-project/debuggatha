import { afterEach, describe, expect, it } from "bun:test";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { inferModuleBoundaries } from "./module-boundaries.js";

let dir: string | undefined;

afterEach(() => {
  if (dir) rmSync(dir, { recursive: true, force: true });
  dir = undefined;
});

function mktemp(): string {
  return mkdtempSync(join(tmpdir(), "debuggatha-analysis-"));
}

describe("inferModuleBoundaries", () => {
  it("prefers a monorepo's own packages/* layout over heuristics", () => {
    dir = mktemp();
    mkdirSync(join(dir, "packages", "a", "src"), { recursive: true });
    mkdirSync(join(dir, "packages", "b", "src"), { recursive: true });
    writeFileSync(join(dir, "packages", "a", "package.json"), JSON.stringify({ name: "@x/a" }));
    writeFileSync(join(dir, "packages", "b", "package.json"), JSON.stringify({ name: "@x/b" }));
    writeFileSync(join(dir, "packages", "a", "src", "index.ts"), "export const a = 1;\n");

    const boundaries = inferModuleBoundaries(dir);
    expect(boundaries.map((b) => b.id)).toEqual(["packages/a", "packages/b"]);
    expect(boundaries[0]?.kind).toBe("package");
    expect(boundaries[0]?.name).toBe("@x/a");
    expect(boundaries[0]?.files).toContain("packages/a/src/index.ts");
  });

  it("falls back to src/<name> directory heuristics for a single-package repo", () => {
    dir = mktemp();
    mkdirSync(join(dir, "src", "domain"), { recursive: true });
    mkdirSync(join(dir, "src", "adapters"), { recursive: true });
    writeFileSync(join(dir, "src", "domain", "user.ts"), "export class User {}\n");
    writeFileSync(join(dir, "src", "adapters", "db.ts"), "export class Db {}\n");

    const boundaries = inferModuleBoundaries(dir);
    expect(boundaries.map((b) => b.id).sort()).toEqual(["src/adapters", "src/domain"]);
    expect(boundaries.find((b) => b.id === "src/adapters")?.kind).toBe("adapter");
    expect(boundaries.find((b) => b.id === "src/domain")?.kind).toBe("domain");
  });

  it("returns no boundaries for a repo with neither a workspace layout nor a src directory", () => {
    dir = mktemp();
    writeFileSync(join(dir, "index.js"), "console.log(1);\n");
    expect(inferModuleBoundaries(dir)).toEqual([]);
  });
});
