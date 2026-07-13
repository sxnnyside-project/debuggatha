import type { ReviewPolicy } from "@debuggatha/knowledge-system";
import { describe, expect, it } from "vitest";
import { reviewDiff } from "./diff-review.js";

const emptyPolicy: ReviewPolicy = { id: "policy-1", rules: [], packRefs: [], conflicts: [] };

const jsPolicy: ReviewPolicy = {
  id: "policy-2",
  rules: [
    {
      id: "no-var",
      statement: "Use `let` or `const` instead of `var`.",
      category: "architecture",
      defaultSeverity: "high",
      origin: { kind: "pack", packId: "debuggatha/javascript", packVersion: "1.0.0" },
    },
  ],
  packRefs: [{ id: "debuggatha/javascript", version: "1.0.0" }],
  conflicts: [],
};

describe("reviewDiff", () => {
  it("returns no findings for an empty policy", () => {
    expect(reviewDiff("--- a/b", emptyPolicy)).toEqual([]);
  });

  it("flags a rule violation introduced by the diff, citing the changed line", () => {
    const diff = `diff --git a/src/foo.js b/src/foo.js
--- a/src/foo.js
+++ b/src/foo.js
@@ -1,2 +1,3 @@
 function foo() {
+  var x = 1;
 }
`;
    const findings = reviewDiff(diff, jsPolicy);
    expect(findings).toHaveLength(1);
    expect(findings[0]?.locations).toEqual([{ file: "src/foo.js", lines: { start: 2, end: 2 } }]);
  });

  it("does not flag a pre-existing violation the diff didn't touch (only + lines are scanned)", () => {
    const diff = `diff --git a/src/foo.js b/src/foo.js
--- a/src/foo.js
+++ b/src/foo.js
@@ -1,3 +1,3 @@
 var alreadyThere = 1;
-function foo() {}
+function foo() { return 1; }
`;
    const findings = reviewDiff(diff, jsPolicy);
    expect(findings).toEqual([]);
  });
});
