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
});
