import { afterEach, describe, expect, it } from "bun:test";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import type { ReviewPolicy } from "@debuggatha/knowledge-system";
import { reviewArchitecture } from "./architecture-review.js";

const emptyPolicy: ReviewPolicy = { id: "policy-1", rules: [], packRefs: [], conflicts: [] };

const jsPolicy: ReviewPolicy = {
  id: "policy-2",
  rules: [
    {
      id: "no-eval",
      statement: "Never use eval().",
      category: "security",
      defaultSeverity: "critical",
      origin: { kind: "pack", packId: "debuggatha/javascript", packVersion: "1.0.0" },
    },
  ],
  packRefs: [{ id: "debuggatha/javascript", version: "1.0.0" }],
  conflicts: [],
};

let dir: string | undefined;

afterEach(() => {
  if (dir) rmSync(dir, { recursive: true, force: true });
  dir = undefined;
});

describe("reviewArchitecture", () => {
  it("returns no findings for an empty policy over an empty repo", () => {
    dir = mkdtempSync(join(tmpdir(), "debuggatha-arch-"));
    expect(reviewArchitecture(dir, emptyPolicy)).toEqual([]);
  });

  it("scans the whole tree and flags a violation regardless of which file it's in", () => {
    dir = mkdtempSync(join(tmpdir(), "debuggatha-arch-"));
    mkdirSync(join(dir, "src", "nested"), { recursive: true });
    writeFileSync(join(dir, "src", "nested", "risky.js"), "eval(userInput);\n");
    writeFileSync(join(dir, "src", "safe.js"), "const x = 1;\n");

    const findings = reviewArchitecture(dir, jsPolicy);
    expect(findings).toHaveLength(1);
    expect(findings[0]?.locations[0]?.file).toBe(join("src", "nested", "risky.js"));
  });

  it("skips node_modules and dist directories", () => {
    dir = mkdtempSync(join(tmpdir(), "debuggatha-arch-"));
    mkdirSync(join(dir, "node_modules", "pkg"), { recursive: true });
    mkdirSync(join(dir, "dist"), { recursive: true });
    writeFileSync(join(dir, "node_modules", "pkg", "index.js"), "eval(x);\n");
    writeFileSync(join(dir, "dist", "bundle.js"), "eval(x);\n");

    expect(reviewArchitecture(dir, jsPolicy)).toEqual([]);
  });
});
