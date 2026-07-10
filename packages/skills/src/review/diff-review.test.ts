import type { ReviewPolicy } from "@debuggatha/knowledge-system";
import { describe, expect, it } from "vitest";
import { reviewDiff } from "./diff-review.js";

const emptyPolicy: ReviewPolicy = { id: "policy-1", rules: [], packRefs: [], conflicts: [] };

describe("reviewDiff", () => {
  it("returns an empty array as a deferred implementation", () => {
    expect(reviewDiff("--- a/b", emptyPolicy)).toEqual([]);
  });
});
