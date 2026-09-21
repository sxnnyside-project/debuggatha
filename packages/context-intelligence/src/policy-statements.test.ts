import { describe, expect, it } from "bun:test";
import { conventionsToPolicyStatements } from "./policy-statements.js";
import type { ContextItem } from "./types.js";

describe("conventionsToPolicyStatements", () => {
  it("extracts only engineering-convention items as plain strings", () => {
    const items: ContextItem[] = [
      {
        id: "1",
        category: "engineering-convention",
        confidence: "documented",
        evidence: [],
        source: "CONTRIBUTING.md",
        rule: "Prefer small functions",
      },
      {
        id: "2",
        category: "documentation",
        confidence: "documented",
        evidence: [],
        source: "README.md",
        kind: "readme",
        title: undefined,
      },
    ];
    expect(conventionsToPolicyStatements(items)).toEqual(["Prefer small functions"]);
  });

  it("returns an empty array when there are no conventions", () => {
    expect(conventionsToPolicyStatements([])).toEqual([]);
  });
});
