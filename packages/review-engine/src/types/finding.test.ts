import { describe, expect, it } from "vitest";
import { CATEGORIES } from "./category.js";
import { affectedFiles, createFinding } from "./finding.js";

const validLocations = [{ file: "src/index.ts", lines: { start: 1, end: 1 } }];
const validEvidence = [
  { kind: "code" as const, file: "src/index.ts", lines: { start: 1, end: 1 }, excerpt: "unwrap()" },
];

describe("createFinding", () => {
  it("builds a Finding with a generated id when none is provided", () => {
    const finding = createFinding({
      title: "Unhandled unwrap()",
      explanation: "Panics on None instead of propagating the error.",
      severity: "high",
      confidence: "high",
      category: CATEGORIES.Reliability,
      locations: validLocations,
      evidence: validEvidence,
    });

    expect(finding.id).toBeTruthy();
    expect(finding.recommendations).toEqual([]);
  });

  it("refuses to create a finding with no evidence", () => {
    expect(() =>
      createFinding({
        title: "Unsupported opinion",
        explanation: "This looks bad.",
        severity: "low",
        confidence: "low",
        category: CATEGORIES.Maintainability,
        locations: validLocations,
        evidence: [],
      }),
    ).toThrow(/never opines without evidence/);
  });

  it("refuses to create a finding with no locations", () => {
    expect(() =>
      createFinding({
        title: "Floating finding",
        explanation: "Points at nothing.",
        severity: "low",
        confidence: "low",
        category: CATEGORIES.Maintainability,
        locations: [],
        evidence: validEvidence,
      }),
    ).toThrow(/must point at something/);
  });

  it("allows independent severity and confidence combinations", () => {
    const criticalButUnverified = createFinding({
      title: "Possible SQL injection",
      explanation: "String concatenation into a query — unverified without a live exploit.",
      severity: "critical",
      confidence: "low",
      category: CATEGORIES.Security,
      locations: validLocations,
      evidence: validEvidence,
    });

    expect(criticalButUnverified.severity).toBe("critical");
    expect(criticalButUnverified.confidence).toBe("low");
  });
});

describe("affectedFiles", () => {
  it("dedupes files across multiple locations", () => {
    const finding = createFinding({
      title: "Repeated pattern",
      explanation: "Same issue in two spots of the same file.",
      severity: "medium",
      confidence: "high",
      category: CATEGORIES.Maintainability,
      locations: [
        { file: "src/index.ts", lines: { start: 1, end: 1 } },
        { file: "src/index.ts", lines: { start: 20, end: 20 } },
      ],
      evidence: validEvidence,
    });

    expect(affectedFiles(finding)).toEqual(["src/index.ts"]);
  });
});
