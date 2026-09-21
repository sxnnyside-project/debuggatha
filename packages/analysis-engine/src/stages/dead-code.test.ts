import { afterEach, describe, expect, it } from "bun:test";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { detectDeadCode } from "./dead-code.js";
import { inferModuleBoundaries } from "./module-boundaries.js";
import { buildPublicApiSurface } from "./public-api.js";

let dir: string | undefined;

afterEach(() => {
  if (dir) rmSync(dir, { recursive: true, force: true });
  dir = undefined;
});

function mktemp(): string {
  return mkdtempSync(join(tmpdir(), "debuggatha-deadcode-"));
}

describe("detectDeadCode", () => {
  it("flags an exported symbol referenced nowhere else as exported-but-unused", () => {
    dir = mktemp();
    mkdirSync(join(dir, "packages", "a", "src"), { recursive: true });
    writeFileSync(join(dir, "packages", "a", "package.json"), JSON.stringify({ name: "@x/a" }));
    writeFileSync(
      join(dir, "packages", "a", "src", "index.ts"),
      "export function unusedThing() {}\n",
    );

    const boundaries = inferModuleBoundaries(dir);
    const apis = boundaries.map((b) => buildPublicApiSurface(dir as string, b));
    const findings = detectDeadCode(dir, boundaries, apis);

    expect(findings).toHaveLength(1);
    expect(findings[0]).toMatchObject({ symbol: "unusedThing", kind: "exported-but-unused" });
  });

  it("does not flag a symbol referenced elsewhere in the repository", () => {
    dir = mktemp();
    mkdirSync(join(dir, "packages", "a", "src"), { recursive: true });
    mkdirSync(join(dir, "packages", "b", "src"), { recursive: true });
    writeFileSync(join(dir, "packages", "a", "package.json"), JSON.stringify({ name: "@x/a" }));
    writeFileSync(join(dir, "packages", "b", "package.json"), JSON.stringify({ name: "@x/b" }));
    writeFileSync(
      join(dir, "packages", "a", "src", "index.ts"),
      "export function usedThing() {}\n",
    );
    writeFileSync(
      join(dir, "packages", "b", "src", "index.ts"),
      'import { usedThing } from "@x/a";\nusedThing();\n',
    );

    const boundaries = inferModuleBoundaries(dir);
    const apis = boundaries.map((b) => buildPublicApiSurface(dir as string, b));
    const findings = detectDeadCode(dir, boundaries, apis);
    expect(findings).toEqual([]);
  });

  it("uses low confidence for probably-dead (non-entrypoint) findings, medium for exported-but-unused", () => {
    dir = mktemp();
    mkdirSync(join(dir, "packages", "a", "src"), { recursive: true });
    writeFileSync(join(dir, "packages", "a", "package.json"), JSON.stringify({ name: "@x/a" }));
    writeFileSync(join(dir, "packages", "a", "src", "index.ts"), "export function pub() {}\n");
    writeFileSync(
      join(dir, "packages", "a", "src", "internal.ts"),
      "export function leaked() {}\n",
    );

    const boundaries = inferModuleBoundaries(dir);
    const apis = boundaries.map((b) => buildPublicApiSurface(dir as string, b));
    const findings = detectDeadCode(dir, boundaries, apis);

    const pubFinding = findings.find((f) => f.symbol === "pub");
    const leakedFinding = findings.find((f) => f.symbol === "leaked");
    expect(pubFinding?.kind).toBe("exported-but-unused");
    expect(pubFinding?.confidence).toBe("medium");
    expect(leakedFinding?.kind).toBe("probably-dead");
    expect(leakedFinding?.confidence).toBe("low");
  });
});
