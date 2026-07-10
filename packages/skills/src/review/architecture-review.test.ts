import type { ReviewPolicy } from "@debuggatha/knowledge-system";
import { describe, expect, it } from "vitest";
import { reviewArchitecture } from "./architecture-review.js";

const emptyPolicy: ReviewPolicy = { id: "policy-1", rules: [], packRefs: [], conflicts: [] };

describe("reviewArchitecture", () => {
  it("returns an empty array as a deferred implementation", () => {
    expect(reviewArchitecture("/repo", emptyPolicy)).toEqual([]);
  });
});
