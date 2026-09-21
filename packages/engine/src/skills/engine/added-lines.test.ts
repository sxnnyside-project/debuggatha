import { describe, expect, test } from "bun:test";
import { addedLinesByFile } from "./diff-parser.js";

describe("addedLinesByFile", () => {
  test("maps each file to the line numbers the diff adds, not its context or removals", () => {
    const diff = [
      "diff --git a/src/a.ts b/src/a.ts",
      "--- a/src/a.ts",
      "+++ b/src/a.ts",
      "@@ -1,3 +1,4 @@",
      " keep",
      "-old",
      "+new one",
      "+new two",
      " tail",
      "@@ -20,0 +22,1 @@",
      "+late",
      "--- a/src/b.ts",
      "+++ b/src/b.ts",
      "@@ -0,0 +1 @@",
      "+fresh",
      "",
    ].join("\n");
    const lines = addedLinesByFile(diff);
    expect([...(lines.get("src/a.ts") ?? [])]).toEqual([2, 3, 22]);
    expect([...(lines.get("src/b.ts") ?? [])]).toEqual([1]);
  });

  test("a change to a file the project does not own is not part of the change", () => {
    const diff = "--- a/node_modules/x/i.js\n+++ b/node_modules/x/i.js\n@@ -1 +1 @@\n+x\n";
    expect(addedLinesByFile(diff).size).toBe(0);
  });
});
