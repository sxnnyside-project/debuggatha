import { buildRepositoryContext } from "@debuggatha/repository-intelligence";
import { withTempRepo } from "@debuggatha/testing";
import { afterEach, describe, expect, it } from "vitest";
import { buildConventionItems } from "./conventions.js";

let cleanup: (() => void) | undefined;
afterEach(() => {
  cleanup?.();
  cleanup = undefined;
});

describe("buildConventionItems", () => {
  it("retypes a repository criteria rule into a documented engineering-convention item", () => {
    const repo = withTempRepo({
      "package.json": JSON.stringify({ name: "demo" }),
      "CONTRIBUTING.md":
        "# Contributing\n\n## Prefer small functions\n\nKeep functions under 20 lines.\n",
    });
    cleanup = repo.cleanup;

    const context = buildRepositoryContext(repo.root);
    const items = buildConventionItems(context.criteria);

    expect(items.length).toBeGreaterThan(0);
    const item = items.find((i) => i.rule.includes("Prefer small functions"));
    expect(item).toBeDefined();
    expect(item?.category).toBe("engineering-convention");
    expect(item?.confidence).toBe("documented");
    expect(item?.evidence[0]?.file).toBe("CONTRIBUTING.md");
  });

  it("produces no items when the repository has no rule-shaped documentation", () => {
    const repo = withTempRepo({ "package.json": JSON.stringify({ name: "demo" }) });
    cleanup = repo.cleanup;
    const context = buildRepositoryContext(repo.root);
    expect(buildConventionItems(context.criteria)).toEqual([]);
  });
});
