import { describe, expect, it } from "bun:test";
import { CATEGORIES, createFinding } from "../../review-engine/index.js";
import { computeFindingFingerprint } from "./fingerprint.js";

describe("computeFindingFingerprint", () => {
  it("extracts the file from the first location", () => {
    const finding = createFinding({
      title: "x",
      explanation: "x",
      severity: "low",
      confidence: "high",
      category: CATEGORIES.Maintainability,
      locations: [{ file: "src/a.ts", lines: undefined }],
      evidence: [{ kind: "language-convention", language: "TypeScript", detail: "x" }],
    });

    expect(computeFindingFingerprint(finding).file).toBe("src/a.ts");
  });

  it("prefers a review-pack-rule ruleId over other evidence kinds", () => {
    const finding = createFinding({
      title: "x",
      explanation: "x",
      severity: "low",
      confidence: "high",
      category: CATEGORIES.Security,
      locations: [{ file: "a.rs", lines: undefined }],
      evidence: [
        { kind: "review-pack-rule", packId: "rust-pack", ruleId: "no-unwrap" },
        { kind: "language-convention", language: "Rust", detail: "x" },
      ],
    });

    expect(computeFindingFingerprint(finding).ruleId).toBe("no-unwrap");
  });

  it("falls back to a criteria ruleId when there is no review-pack-rule evidence", () => {
    const finding = createFinding({
      title: "x",
      explanation: "x",
      severity: "low",
      confidence: "high",
      category: CATEGORIES.Maintainability,
      locations: [{ file: "a.ts", lines: undefined }],
      evidence: [{ kind: "criteria", ruleId: "eslint:no-console", source: ".eslintrc.json" }],
    });

    expect(computeFindingFingerprint(finding).ruleId).toBe("eslint:no-console");
  });

  it("leaves ruleId undefined when no rule-shaped evidence is present", () => {
    const finding = createFinding({
      title: "x",
      explanation: "x",
      severity: "low",
      confidence: "high",
      category: CATEGORIES.Maintainability,
      locations: [{ file: "a.ts", lines: undefined }],
      evidence: [{ kind: "documentation", file: "README.md", excerpt: "x" }],
    });

    expect(computeFindingFingerprint(finding).ruleId).toBeUndefined();
  });

  it("computes a stable content anchor from code evidence, and none when no excerpt is present", () => {
    const withExcerpt = createFinding({
      title: "x",
      explanation: "x",
      severity: "low",
      confidence: "high",
      category: CATEGORIES.Reliability,
      locations: [{ file: "a.ts", lines: { start: 1, end: 1 } }],
      evidence: [{ kind: "code", file: "a.ts", lines: { start: 1, end: 1 }, excerpt: "unwrap()" }],
    });
    const withoutExcerpt = createFinding({
      title: "x",
      explanation: "x",
      severity: "low",
      confidence: "high",
      category: CATEGORIES.Reliability,
      locations: [{ file: "a.ts", lines: { start: 1, end: 1 } }],
      evidence: [{ kind: "code", file: "a.ts", lines: undefined, excerpt: undefined }],
    });

    const fp1 = computeFindingFingerprint(withExcerpt);
    const fp2 = computeFindingFingerprint(withExcerpt);
    expect(fp1.contentAnchor).toBeDefined();
    expect(fp1.contentAnchor).toBe(fp2.contentAnchor);
    expect(computeFindingFingerprint(withoutExcerpt).contentAnchor).toBeUndefined();
  });
});
