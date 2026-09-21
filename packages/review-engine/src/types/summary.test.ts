import { describe, expect, it } from "bun:test";
import { CATEGORIES } from "./category.js";
import { createFinding } from "./finding.js";
import { summarizeFindings } from "./summary.js";

const evidence = [{ kind: "code" as const, file: "a.ts", lines: undefined, excerpt: undefined }];
const locations = [{ file: "a.ts", lines: undefined }];

describe("summarizeFindings", () => {
  it("counts findings by severity, including zero counts for severities that did not occur", () => {
    const findings = [
      createFinding({
        title: "A",
        explanation: "x",
        severity: "critical",
        confidence: "high",
        category: CATEGORIES.Security,
        locations,
        evidence,
      }),
      createFinding({
        title: "B",
        explanation: "x",
        severity: "critical",
        confidence: "medium",
        category: CATEGORIES.Security,
        locations,
        evidence,
      }),
      createFinding({
        title: "C",
        explanation: "x",
        severity: "low",
        confidence: "high",
        category: CATEGORIES.Maintainability,
        locations,
        evidence,
      }),
    ];

    const summary = summarizeFindings(findings);

    expect(summary.totalFindings).toBe(3);
    expect(summary.findingsBySeverity).toEqual({
      critical: 2,
      high: 0,
      medium: 0,
      low: 1,
      informational: 0,
    });
  });

  it("counts findings by category, an open key set", () => {
    const findings = [
      createFinding({
        title: "A",
        explanation: "x",
        severity: "medium",
        confidence: "high",
        category: "supply-chain",
        locations,
        evidence,
      }),
    ];

    const summary = summarizeFindings(findings);

    expect(summary.findingsByCategory["supply-chain"]).toBe(1);
  });

  it("carries through duration and applied packs/policies unchanged", () => {
    const summary = summarizeFindings([], {
      durationMs: 4200,
      appliedPacks: [{ id: "react-pack", version: "1.0.0" }],
      appliedPolicies: [{ id: "policy-1" }],
    });

    expect(summary.durationMs).toBe(4200);
    expect(summary.appliedPacks).toEqual([{ id: "react-pack", version: "1.0.0" }]);
    expect(summary.appliedPolicies).toEqual([{ id: "policy-1" }]);
  });

  it("returns a zeroed summary for an empty findings list", () => {
    const summary = summarizeFindings([]);
    expect(summary.totalFindings).toBe(0);
    expect(Object.values(summary.findingsBySeverity).every((count) => count === 0)).toBe(true);
  });
});
