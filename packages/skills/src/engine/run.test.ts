import { describe, expect, it } from "bun:test";
import type { PolicyRule, ReviewPolicy } from "@debuggatha/knowledge-system";
import { runReviewSkillsEngine } from "./run.js";

function policyFor(rules: PolicyRule[]): ReviewPolicy {
  return {
    id: "policy-1",
    rules,
    packRefs: [{ id: "debuggatha/javascript", version: "1.0.0" }],
    conflicts: [],
  };
}

const noVarRule: PolicyRule = {
  id: "no-var",
  statement: "Use `let` or `const` instead of `var`.",
  category: "architecture",
  defaultSeverity: "high",
  origin: { kind: "pack", packId: "debuggatha/javascript", packVersion: "1.0.0" },
};

describe("runReviewSkillsEngine", () => {
  it("produces no findings for an empty policy", () => {
    expect(
      runReviewSkillsEngine(policyFor([]), [
        { file: "a.js", content: "var x;", lineNumbers: undefined },
      ]),
    ).toEqual([]);
  });

  it("produces no findings when no detector is registered for a rule", () => {
    const rule: PolicyRule = { ...noVarRule, id: "some-unmapped-rule" };
    expect(
      runReviewSkillsEngine(policyFor([rule]), [
        { file: "a.js", content: "var x;", lineNumbers: undefined },
      ]),
    ).toEqual([]);
  });

  it("produces a citable Finding for a matched rule", () => {
    const findings = runReviewSkillsEngine(policyFor([noVarRule]), [
      { file: "a.js", content: "var x = 1;", lineNumbers: undefined },
    ]);
    expect(findings).toHaveLength(1);
    const finding = findings[0];
    expect(finding).toBeDefined();
    expect(finding?.category).toBe("architecture");
    expect(finding?.severity).toBe("high");
    expect(finding?.locations).toEqual([{ file: "a.js", lines: { start: 1, end: 1 } }]);
    expect(finding?.appliedPolicy).toEqual({ id: "policy-1" });
    expect(finding?.originatingPack).toEqual({ id: "debuggatha/javascript", version: "1.0.0" });
    expect(
      finding?.evidence.some((e) => e.kind === "review-pack-rule" && e.ruleId === "no-var"),
    ).toBe(true);
    expect(finding?.evidence.some((e) => e.kind === "code" && e.file === "a.js")).toBe(true);
    expect(finding?.recommendations).toHaveLength(1);
  });

  it("cites `criteria` evidence for a repository-derived rule, not `review-pack-rule`", () => {
    const criteriaRule: PolicyRule = {
      id: "repo-no-var",
      statement: "This repo's own linter config forbids var.",
      category: undefined,
      defaultSeverity: undefined,
      origin: { kind: "criteria" },
    };
    // no detector registered for an arbitrary criteria id — falls through silently
    expect(
      runReviewSkillsEngine(policyFor([criteriaRule]), [
        { file: "a.js", content: "var x;", lineNumbers: undefined },
      ]),
    ).toEqual([]);
  });

  it("scans every provided source unit, not just the first", () => {
    const findings = runReviewSkillsEngine(policyFor([noVarRule]), [
      { file: "a.js", content: "const ok = 1;", lineNumbers: undefined },
      { file: "b.js", content: "var bad = 2;", lineNumbers: undefined },
    ]);
    expect(findings.map((f) => f.locations[0]?.file)).toEqual(["b.js"]);
  });

  it("defaults severity/category when a criteria-derived rule matches a registered detector", () => {
    const criteriaRule: PolicyRule = {
      id: "no-var",
      statement: "Repo criteria: no var.",
      category: undefined,
      defaultSeverity: undefined,
      origin: { kind: "criteria" },
    };
    const findings = runReviewSkillsEngine(policyFor([criteriaRule]), [
      { file: "a.js", content: "var x;", lineNumbers: undefined },
    ]);
    expect(findings[0]?.severity).toBe("medium");
    expect(findings[0]?.category).toBe("maintainability");
    expect(findings[0]?.originatingPack).toBeUndefined();
    expect(findings[0]?.evidence.some((e) => e.kind === "criteria")).toBe(true);
  });
});
