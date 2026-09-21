import { describe, expect, it } from "bun:test";
import type { Category } from "./category.js";
import { CATEGORIES } from "./category.js";
import type { Evidence } from "./evidence.js";
import type { Confidence, Severity } from "./severity.js";
import { CONFIDENCE_LEVELS, SEVERITIES } from "./severity.js";

describe("Severity and Confidence", () => {
  it("are independent value sets — Severity has five levels, Confidence has three", () => {
    expect(SEVERITIES).toEqual(["critical", "high", "medium", "low", "informational"]);
    expect(CONFIDENCE_LEVELS).toEqual(["high", "medium", "low"]);
  });

  it("share no conversion function between them by design", () => {
    // A finding can be Critical severity with Low confidence, or Low
    // severity with High confidence — both are valid combinations, since
    // nothing in this module derives one from the other.
    const severity: Severity = "critical";
    const confidence: Confidence = "low";
    expect(severity).toBe("critical");
    expect(confidence).toBe("low");
  });
});

describe("Category", () => {
  it("accepts every well-known category constant", () => {
    const known: Category[] = Object.values(CATEGORIES);
    expect(known).toContain("architecture");
    expect(known).toContain("developer-experience");
  });

  it("stays extensible — an arbitrary string is a valid Category", () => {
    const custom: Category = "supply-chain";
    expect(custom).toBe("supply-chain");
  });
});

describe("Evidence", () => {
  it("is a machine-readable discriminated union covering every source named in the epic", () => {
    const evidence: Evidence[] = [
      { kind: "documentation", file: "ARCHITECTURE.md", excerpt: "Layered core/adapters" },
      { kind: "criteria", ruleId: "eslint:no-console", source: ".eslintrc.json" },
      { kind: "review-pack-rule", packId: "rust-pack", ruleId: "no-unwrap-in-prod" },
      { kind: "framework-convention", framework: "React", detail: "Hooks must not be conditional" },
      { kind: "language-convention", language: "Rust", detail: "Prefer `?` over `.unwrap()`" },
      { kind: "code", file: "src/index.ts", lines: { start: 10, end: 12 }, excerpt: "unwrap()" },
    ];

    expect(evidence).toHaveLength(6);
    expect(new Set(evidence.map((e) => e.kind)).size).toBe(6);
  });
});
