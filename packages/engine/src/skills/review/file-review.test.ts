import { afterEach, describe, expect, it } from "bun:test";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import type { ReviewPolicy } from "@debuggatha/core";
import { reviewFiles } from "./file-review.js";

const tsPolicy: ReviewPolicy = {
  id: "policy-1",
  rules: [
    {
      id: "no-explicit-any",
      statement: "Avoid using the `any` type.",
      category: "architecture",
      defaultSeverity: "high",
      origin: { kind: "pack", packId: "debuggatha/typescript", packVersion: "1.0.0" },
    },
  ],
  packRefs: [{ id: "debuggatha/typescript", version: "1.0.0" }],
  conflicts: [],
};

let dir: string | undefined;

afterEach(() => {
  if (dir) rmSync(dir, { recursive: true, force: true });
  dir = undefined;
});

describe("reviewFiles", () => {
  it("reviews only the named files, in full", () => {
    dir = mkdtempSync(join(tmpdir(), "debuggatha-file-"));
    const target = join(dir, "a.ts");
    writeFileSync(target, "function f(x: any) {\n  return x;\n}\n");
    writeFileSync(join(dir, "b.ts"), "function g(x: any) { return x; }\n");

    const findings = reviewFiles([target], tsPolicy);
    expect(findings).toHaveLength(1);
    expect(findings[0]?.locations[0]?.file).toBe(target);
  });

  it("skips unreadable paths instead of throwing", () => {
    expect(() => reviewFiles(["/does/not/exist.ts"], tsPolicy)).not.toThrow();
    expect(reviewFiles(["/does/not/exist.ts"], tsPolicy)).toEqual([]);
  });
});
