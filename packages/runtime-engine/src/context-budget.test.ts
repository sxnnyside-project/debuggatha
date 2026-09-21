import { describe, expect, it } from "bun:test";
import { budgetSourceUnits, chunkSourceUnit, estimateTokens } from "./context-budget.js";

describe("estimateTokens", () => {
  it("estimates roughly 4 characters per token", () => {
    expect(estimateTokens("a".repeat(400))).toBe(100);
  });
});

describe("budgetSourceUnits", () => {
  it("includes every unit when the budget is generous", () => {
    const units = [
      { file: "a.ts", content: "short" },
      { file: "b.ts", content: "also short" },
    ];
    const result = budgetSourceUnits(units, { contextWindow: 10_000, reservedTokens: 0 });
    expect(result.included).toEqual(units);
    expect(result.excluded).toEqual([]);
  });

  it("excludes units that would exceed the budget, reporting why", () => {
    const units = [
      { file: "small.ts", content: "x".repeat(40) },
      { file: "huge.ts", content: "y".repeat(4000) },
    ];
    const result = budgetSourceUnits(units, { contextWindow: 100, reservedTokens: 0 });
    expect(result.included.map((u) => u.file)).toEqual(["small.ts"]);
    expect(result.excluded).toHaveLength(1);
    expect(result.excluded[0]?.file).toBe("huge.ts");
    expect(result.excluded[0]?.reason).toMatch(/exceeds/);
  });

  it("falls back to a conservative default context window when none is known", () => {
    const units = [{ file: "a.ts", content: "x".repeat(100) }];
    const result = budgetSourceUnits(units, { contextWindow: undefined, reservedTokens: 0 });
    expect(result.included).toHaveLength(1);
  });

  it("never produces a negative available budget", () => {
    const units = [{ file: "a.ts", content: "x" }];
    const result = budgetSourceUnits(units, { contextWindow: 100, reservedTokens: 10_000 });
    expect(result.included).toEqual([]);
    expect(result.excluded).toHaveLength(1);
  });
});

describe("chunkSourceUnit", () => {
  it("splits a large unit into multiple line-ranged chunks", () => {
    const lines = Array.from({ length: 100 }, (_, i) => `line ${i}`);
    const unit = { file: "big.ts", content: lines.join("\n") };
    const chunks = chunkSourceUnit(unit, 20); // ~80 chars per chunk
    expect(chunks.length).toBeGreaterThan(1);
    expect(chunks[0]?.file).toMatch(/^big\.ts:1-/);
    expect(chunks.every((c) => c.content.length > 0)).toBe(true);
  });

  it("returns a single chunk for content that fits", () => {
    const unit = { file: "small.ts", content: "line one\nline two" };
    const chunks = chunkSourceUnit(unit, 1000);
    expect(chunks).toHaveLength(1);
    expect(chunks[0]?.content).toBe(unit.content);
  });
});
