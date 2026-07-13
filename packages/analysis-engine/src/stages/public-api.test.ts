import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import { inferModuleBoundaries } from "./module-boundaries.js";
import { buildPublicApiSurface } from "./public-api.js";

let dir: string | undefined;

afterEach(() => {
  if (dir) rmSync(dir, { recursive: true, force: true });
  dir = undefined;
});

function mktemp(): string {
  return mkdtempSync(join(tmpdir(), "debuggatha-api-"));
}

describe("buildPublicApiSurface", () => {
  it("treats exports from src/index.ts as the module's public API", () => {
    dir = mktemp();
    mkdirSync(join(dir, "packages", "a", "src"), { recursive: true });
    writeFileSync(join(dir, "packages", "a", "package.json"), JSON.stringify({ name: "@x/a" }));
    writeFileSync(
      join(dir, "packages", "a", "src", "index.ts"),
      "export function doThing() {}\nexport const VALUE = 1;\n",
    );

    const boundary = inferModuleBoundaries(dir)[0];
    expect(boundary).toBeDefined();
    const surface = boundary && buildPublicApiSurface(dir, boundary);
    expect(surface?.entrypoint).toBe("packages/a/src/index.ts");
    expect(surface?.exported.map((s) => s.name).sort()).toEqual(["VALUE", "doThing"]);
    expect(surface?.unexpectedlyExposed).toEqual([]);
  });

  it("flags exports from a non-entrypoint file as unexpectedly exposed", () => {
    dir = mktemp();
    mkdirSync(join(dir, "packages", "a", "src"), { recursive: true });
    writeFileSync(join(dir, "packages", "a", "package.json"), JSON.stringify({ name: "@x/a" }));
    writeFileSync(join(dir, "packages", "a", "src", "index.ts"), "export function pub() {}\n");
    writeFileSync(
      join(dir, "packages", "a", "src", "internal.ts"),
      "export function leaked() {}\n",
    );

    const boundary = inferModuleBoundaries(dir)[0];
    const surface = boundary && buildPublicApiSurface(dir, boundary);
    expect(surface?.exported.map((s) => s.name)).toEqual(["pub"]);
    expect(surface?.unexpectedlyExposed.map((s) => s.name)).toEqual(["leaked"]);
  });
});
