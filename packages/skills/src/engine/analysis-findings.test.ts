import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import { buildAnalysisFindings } from "./analysis-findings.js";

let dir: string | undefined;

afterEach(() => {
  if (dir) rmSync(dir, { recursive: true, force: true });
  dir = undefined;
});

function mktemp(): string {
  return mkdtempSync(join(tmpdir(), "debuggatha-analysis-findings-"));
}

describe("buildAnalysisFindings", () => {
  it("turns a layer violation from the analysis engine into a citable Finding", () => {
    dir = mktemp();
    mkdirSync(join(dir, "packages", "domain-users", "src"), { recursive: true });
    mkdirSync(join(dir, "packages", "adapters-db", "src"), { recursive: true });
    writeFileSync(
      join(dir, "packages", "domain-users", "package.json"),
      JSON.stringify({
        name: "@x/domain-users",
        dependencies: { "@x/adapters-db": "workspace:*" },
      }),
    );
    writeFileSync(
      join(dir, "packages", "adapters-db", "package.json"),
      JSON.stringify({ name: "@x/adapters-db" }),
    );
    writeFileSync(
      join(dir, "packages", "domain-users", "src", "index.ts"),
      "export function getUser() {}\n",
    );
    writeFileSync(join(dir, "packages", "adapters-db", "src", "index.ts"), "export class Db {}\n");

    const findings = buildAnalysisFindings(dir);
    expect(findings).toHaveLength(1);
    expect(findings[0]?.category).toBe("architecture");
    expect(findings[0]?.title).toContain("domain-users");
    expect(findings[0]?.evidence.some((e) => e.kind === "framework-convention")).toBe(true);
  });

  it("returns no findings for a repo with no layer violations or cycles", () => {
    dir = mktemp();
    mkdirSync(join(dir, "packages", "a", "src"), { recursive: true });
    writeFileSync(join(dir, "packages", "a", "package.json"), JSON.stringify({ name: "@x/a" }));
    writeFileSync(join(dir, "packages", "a", "src", "index.ts"), "export const a = 1;\n");
    expect(buildAnalysisFindings(dir)).toEqual([]);
  });
});
