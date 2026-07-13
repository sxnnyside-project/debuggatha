import { validateReviewPack } from "@debuggatha/knowledge-system";
import { describe, expect, it } from "vitest";
import { reviewPacks } from "./index.js";

describe("reviewPacks catalog", () => {
  it("every registered pack passes structural validation", () => {
    for (const pack of reviewPacks) {
      expect(() => validateReviewPack(pack)).not.toThrow();
    }
  });

  it("exports typescriptPack as part of the catalog", () => {
    expect(reviewPacks.some((p) => p.id === "debuggatha/typescript")).toBe(true);
  });

  // Epic 16B (Deferred Inventory item K): `validateReviewPack` already
  // checks *within*-pack invariants (unique rule/knowledge ids, valid
  // knowledgeRefs, non-empty limitations — see
  // `@debuggatha/knowledge-system`'s `validate.ts`). What it can't check,
  // because it only ever sees one pack at a time, is catalog-wide
  // consistency — added here rather than duplicating any of its logic.
  it("has no duplicate pack ids across the whole catalog", () => {
    const ids = reviewPacks.map((p) => p.id);
    expect(new Set(ids).size).toBe(ids.length);
  });

  it("every pack declares a non-empty displayName and at least one rule", () => {
    for (const pack of reviewPacks) {
      expect(
        pack.displayName.trim().length,
        `pack "${pack.id}" has an empty displayName`,
      ).toBeGreaterThan(0);
      expect(pack.rules.length, `pack "${pack.id}" has no rules`).toBeGreaterThan(0);
    }
  });

  it("every rule statement is non-empty prose, not a placeholder", () => {
    for (const pack of reviewPacks) {
      for (const rule of pack.rules) {
        expect(
          rule.statement.trim().length,
          `rule "${rule.id}" in pack "${pack.id}" has an empty statement`,
        ).toBeGreaterThan(10);
      }
    }
  });
});
