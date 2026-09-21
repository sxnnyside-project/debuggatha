import { afterEach, describe, expect, it } from "bun:test";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { inferModuleBoundaries } from "./module-boundaries.js";
import { detectOwnership } from "./ownership.js";

let dir: string | undefined;

afterEach(() => {
  if (dir) rmSync(dir, { recursive: true, force: true });
  dir = undefined;
});

function mktemp(): string {
  return mkdtempSync(join(tmpdir(), "debuggatha-ownership-"));
}

describe("detectOwnership", () => {
  it("reports unknown ownership when there is no CODEOWNERS file", () => {
    dir = mktemp();
    mkdirSync(join(dir, "packages", "a", "src"), { recursive: true });
    writeFileSync(join(dir, "packages", "a", "package.json"), JSON.stringify({ name: "@x/a" }));

    const boundaries = inferModuleBoundaries(dir);
    const ownership = detectOwnership(dir, boundaries);
    expect(ownership).toEqual([{ moduleId: "packages/a", owners: [], source: "unknown" }]);
  });

  it("resolves owners from CODEOWNERS, last matching rule wins", () => {
    dir = mktemp();
    mkdirSync(join(dir, "packages", "a", "src"), { recursive: true });
    writeFileSync(join(dir, "packages", "a", "package.json"), JSON.stringify({ name: "@x/a" }));
    writeFileSync(join(dir, "CODEOWNERS"), "packages/* @team-generic\npackages/a @team-a\n");

    const boundaries = inferModuleBoundaries(dir);
    const ownership = detectOwnership(dir, boundaries);
    expect(ownership).toEqual([
      { moduleId: "packages/a", owners: ["@team-a"], source: "codeowners" },
    ]);
  });

  it("finds CODEOWNERS under .github/ too", () => {
    dir = mktemp();
    mkdirSync(join(dir, "packages", "a", "src"), { recursive: true });
    mkdirSync(join(dir, ".github"), { recursive: true });
    writeFileSync(join(dir, "packages", "a", "package.json"), JSON.stringify({ name: "@x/a" }));
    writeFileSync(join(dir, ".github", "CODEOWNERS"), "packages/a @team-a\n");

    const boundaries = inferModuleBoundaries(dir);
    expect(detectOwnership(dir, boundaries)[0]?.owners).toEqual(["@team-a"]);
  });
});
