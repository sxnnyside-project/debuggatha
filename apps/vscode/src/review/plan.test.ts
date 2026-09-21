import { addedLinesByFile } from "@debuggatha/engine";
import { describe, expect, it } from "vitest";
import { planReview, selectionDiff } from "./plan.js";

const settings = (over: Partial<Parameters<typeof planReview>[1]> = {}) => ({
  reviewDepth: "full" as const,
  enabledAnalyzers: [],
  ...over,
});

describe("planReview", () => {
  it("full runs the analyzers that are safe by themselves, and only the ones the person listed beyond them", () => {
    const plan = planReview(
      { kind: "workspace" },
      settings({ enabledAnalyzers: ["eslint"] }),
      true,
    );
    expect(plan.analyzers).toEqual({ mode: "auto", enable: ["eslint"] });
    expect(plan.scope).toEqual({ kind: "workspace" });
    expect(plan.notes).toEqual([]);
  });

  it("quick runs the built-in detectors only", () => {
    const plan = planReview(
      { kind: "file", path: "/w/a.ts" },
      settings({ reviewDepth: "quick" }),
      true,
    );
    expect(plan.analyzers).toEqual({ mode: "off" });
    expect(plan.scope).toEqual({ kind: "files", paths: ["/w/a.ts"] });
  });

  it("architectural widens a file or a diff to the whole repository, and says so", () => {
    for (const request of [
      { kind: "file", path: "/w/a.ts" },
      { kind: "diff", diff: "x" },
      { kind: "selection", diff: "x", path: "/w/a.ts" },
    ] as const) {
      const plan = planReview(request, settings({ reviewDepth: "architectural" }), true);
      expect(plan.scope).toEqual({ kind: "workspace" });
      expect(plan.widened).toBe(true);
      expect(plan.notes.join(" ")).toContain("whole repository");
      expect(plan.analyzers.mode).toBe("auto");
    }
  });

  it("architectural on a workspace review is not widened, because it already is the whole repository", () => {
    const plan = planReview(
      { kind: "workspace" },
      settings({ reviewDepth: "architectural" }),
      true,
    );
    expect(plan.widened).toBe(false);
    expect(plan.notes).toEqual([]);
  });

  it("an untrusted workspace never runs an analyzer, whatever the depth or the settings", () => {
    for (const reviewDepth of ["quick", "full", "architectural"] as const) {
      const plan = planReview(
        { kind: "workspace" },
        settings({ reviewDepth, enabledAnalyzers: ["eslint", "phpstan"] }),
        false,
      );
      expect(plan.analyzers).toEqual({ mode: "off" });
      expect(plan.notes.join(" ")).toContain("not trusted");
    }
  });

  it("a diff or a selection reviews as a diff", () => {
    expect(planReview({ kind: "diff", diff: "D" }, settings(), true).scope).toEqual({
      kind: "diff",
      base: undefined,
      diff: "D",
    });
    expect(
      planReview({ kind: "selection", diff: "S", path: "/w/a.ts" }, settings(), true).scope,
    ).toEqual({
      kind: "diff",
      base: undefined,
      diff: "S",
    });
  });
});

describe("selectionDiff", () => {
  it("adds exactly the selected lines, at their real line numbers", () => {
    const diff = selectionDiff("src\\a.ts", 41, ["var x = 1;", "  eval(x);"]);
    expect(diff).toBe(
      "--- a/src/a.ts\n+++ b/src/a.ts\n@@ -0,0 +41,2 @@\n+var x = 1;\n+  eval(x);\n",
    );
    // The engine reads it as those two lines, numbered where they are in the file.
    expect(addedLinesByFile(diff)).toEqual(new Map([["src/a.ts", new Set([41, 42])]]));
  });
});
