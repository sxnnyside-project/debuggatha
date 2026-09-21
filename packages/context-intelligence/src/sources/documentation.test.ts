import { afterEach, describe, expect, it } from "bun:test";
import { buildRepositoryContext } from "@debuggatha/repository-intelligence";
import { withTempRepo } from "@debuggatha/testing";
import { buildDocumentationItems } from "./documentation.js";

let cleanup: (() => void) | undefined;
afterEach(() => {
  cleanup?.();
  cleanup = undefined;
});

describe("buildDocumentationItems", () => {
  it("produces one documented item per documentation source, without storing prose", () => {
    const repo = withTempRepo({
      "package.json": JSON.stringify({ name: "demo" }),
      "README.md": "# Demo\n\nA demo project.\n",
      "ARCHITECTURE.md": "# Architecture\n\nLayered.\n",
    });
    cleanup = repo.cleanup;

    const context = buildRepositoryContext(repo.root);
    const items = buildDocumentationItems(context.documentation);

    expect(items.length).toBeGreaterThanOrEqual(2);
    for (const item of items) {
      expect(item.confidence).toBe("documented");
      expect(item.category).toBe("documentation");
      // Structural fact only — no prose/body content stored on the item.
      expect(Object.keys(item)).not.toContain("body");
    }
    expect(items.some((i) => i.source === "README.md")).toBe(true);
  });
});
