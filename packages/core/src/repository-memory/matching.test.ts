import { describe, expect, it } from "bun:test";
import { createFinding } from "../review-engine/index.js";
import { acceptedDeviationMatches, exceptionMatches, suppressionMatches } from "./matching.js";
import type { AcceptedDeviationItem, ExceptionItem, SuppressionItem } from "./types/item.js";

function fixtureFinding(file: string, ruleId: string) {
  return createFinding({
    title: "t",
    explanation: "e",
    severity: "medium",
    confidence: "medium",
    category: "architecture",
    locations: [{ file, lines: undefined }],
    evidence: [{ kind: "review-pack-rule", packId: "p", ruleId }],
  });
}

function baseMeta() {
  return {
    status: "active" as const,
    history: [],
    createdAt: "t",
    updatedAt: "t",
    rationale: "r",
    evidence: [],
  };
}

describe("suppressionMatches", () => {
  it("matches by file + ruleId", () => {
    const item: SuppressionItem = {
      id: "1",
      category: "suppression",
      confidence: "user-confirmed",
      fingerprintFile: "src/a.ts",
      fingerprintRuleId: "no-var",
      fingerprintCategory: "architecture",
      ...baseMeta(),
    };
    expect(suppressionMatches(item, fixtureFinding("src/a.ts", "no-var"))).toBe(true);
    expect(suppressionMatches(item, fixtureFinding("src/a.ts", "no-eval"))).toBe(false);
    expect(suppressionMatches(item, fixtureFinding("src/b.ts", "no-var"))).toBe(false);
  });
});

describe("acceptedDeviationMatches", () => {
  it("matches repository-wide when scopeFile is undefined", () => {
    const item: AcceptedDeviationItem = {
      id: "1",
      category: "accepted-deviation",
      confidence: "user-confirmed",
      ruleId: "no-explicit-any",
      scopeFile: undefined,
      ...baseMeta(),
    };
    expect(acceptedDeviationMatches(item, fixtureFinding("src/a.ts", "no-explicit-any"))).toBe(
      true,
    );
    expect(
      acceptedDeviationMatches(item, fixtureFinding("src/anywhere.ts", "no-explicit-any")),
    ).toBe(true);
    expect(acceptedDeviationMatches(item, fixtureFinding("src/a.ts", "other-rule"))).toBe(false);
  });

  it("matches only the scoped file when scopeFile is set", () => {
    const item: AcceptedDeviationItem = {
      id: "1",
      category: "accepted-deviation",
      confidence: "user-confirmed",
      ruleId: "no-explicit-any",
      scopeFile: "src/legacy.ts",
      ...baseMeta(),
    };
    expect(acceptedDeviationMatches(item, fixtureFinding("src/legacy.ts", "no-explicit-any"))).toBe(
      true,
    );
    expect(acceptedDeviationMatches(item, fixtureFinding("src/other.ts", "no-explicit-any"))).toBe(
      false,
    );
  });
});

describe("exceptionMatches", () => {
  it("matches every rule in scopeFile when ruleId is undefined", () => {
    const item: ExceptionItem = {
      id: "1",
      category: "exception",
      confidence: "documented",
      exceptionKind: "legacy-integration",
      ruleId: undefined,
      scopeFile: "src/legacy/adapter.ts",
      ...baseMeta(),
    };
    expect(exceptionMatches(item, fixtureFinding("src/legacy/adapter.ts", "anything"))).toBe(true);
    expect(exceptionMatches(item, fixtureFinding("src/other.ts", "anything"))).toBe(false);
  });
});
