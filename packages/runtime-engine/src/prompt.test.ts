import { describe, expect, it } from "bun:test";
import { buildSemanticPrompt } from "./prompt.js";

describe("buildSemanticPrompt", () => {
  it("includes response-format instructions and every unit that fits", () => {
    const units = [
      { file: "a.ts", content: "const a = 1;" },
      { file: "b.ts", content: "const b = 2;" },
    ];
    const { prompt, budget } = buildSemanticPrompt(units, {
      contextWindow: 8192,
      reservedTokens: 0,
    });

    expect(prompt).toContain('"candidates"');
    expect(prompt).toContain("--- a.ts ---");
    expect(prompt).toContain("--- b.ts ---");
    expect(budget.included).toHaveLength(2);
    expect(budget.excluded).toEqual([]);
  });

  it("includes repository-specific policy statements when provided", () => {
    const { prompt } = buildSemanticPrompt(
      [{ file: "a.ts", content: "x" }],
      { contextWindow: 8192, reservedTokens: 0 },
      ["Never use var."],
    );
    expect(prompt).toContain("Never use var.");
  });

  it("chunks an oversized unit instead of excluding it wholesale", () => {
    const hugeContent = Array.from({ length: 2000 }, (_, i) => `line ${i}`).join("\n");
    const { prompt, budget } = buildSemanticPrompt([{ file: "huge.ts", content: hugeContent }], {
      contextWindow: 2048,
      reservedTokens: 0,
    });
    expect(budget.included.length).toBeGreaterThan(0);
    expect(prompt).toContain("huge.ts:1-");
  });

  it("reports excluded units when the budget is too small even after chunking", () => {
    const units = Array.from({ length: 50 }, (_, i) => ({
      file: `f${i}.ts`,
      content: "x".repeat(200),
    }));
    const { budget } = buildSemanticPrompt(units, { contextWindow: 512, reservedTokens: 0 });
    expect(budget.excluded.length).toBeGreaterThan(0);
  });
});
