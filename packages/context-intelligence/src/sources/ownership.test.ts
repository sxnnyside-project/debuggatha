import { afterEach, describe, expect, it } from "bun:test";
import { withTempRepo } from "@debuggatha/testing";
import { buildOwnershipItems } from "./ownership.js";

let cleanup: (() => void) | undefined;
afterEach(() => {
  cleanup?.();
  cleanup = undefined;
});

describe("buildOwnershipItems", () => {
  it("emits an ownership item only for a module CODEOWNERS actually attributes", () => {
    const repo = withTempRepo({
      "packages/a/package.json": JSON.stringify({ name: "@x/a" }),
      "packages/b/package.json": JSON.stringify({ name: "@x/b" }),
      CODEOWNERS: "packages/a @team-a\n",
    });
    cleanup = repo.cleanup;

    const items = buildOwnershipItems(repo.root);
    expect(items).toHaveLength(1);
    expect(items[0]?.moduleId).toBe("packages/a");
    expect(items[0]?.owners).toEqual(["@team-a"]);
    expect(items[0]?.confidence).toBe("documented");
  });

  it("emits nothing when there is no CODEOWNERS at all", () => {
    const repo = withTempRepo({
      "packages/a/package.json": JSON.stringify({ name: "@x/a" }),
    });
    cleanup = repo.cleanup;
    expect(buildOwnershipItems(repo.root)).toEqual([]);
  });
});
