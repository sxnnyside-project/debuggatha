import { describe, expect, it } from "bun:test";
import { parseUnifiedDiff } from "./diff-parser.js";

const diff = `diff --git a/src/foo.js b/src/foo.js
index 111..222 100644
--- a/src/foo.js
+++ b/src/foo.js
@@ -1,3 +1,4 @@
 function foo() {
-  return 1;
+  var x = eval(input);
+  return x;
 }
`;

describe("parseUnifiedDiff", () => {
  it("extracts only added lines, mapped to real file line numbers", () => {
    const units = parseUnifiedDiff(diff);
    expect(units).toHaveLength(1);
    expect(units[0]?.file).toBe("src/foo.js");
    expect(units[0]?.content).toBe("  var x = eval(input);\n  return x;");
    expect(units[0]?.lineNumbers).toEqual([2, 3]);
  });

  it("ignores removed and context lines entirely", () => {
    const units = parseUnifiedDiff(diff);
    expect(units[0]?.content).not.toContain("return 1");
    expect(units[0]?.content).not.toContain("function foo");
  });

  it("returns nothing for a diff with no hunks", () => {
    expect(parseUnifiedDiff("")).toEqual([]);
  });
});
