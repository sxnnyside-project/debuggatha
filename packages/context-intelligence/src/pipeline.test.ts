import { afterEach, describe, expect, it } from "bun:test";
import { writeFileSync } from "node:fs";
import { join } from "node:path";
import { buildRepositoryContext } from "@debuggatha/repository-intelligence";
import { withTempRepo } from "@debuggatha/testing";
import { createInMemoryContextCache } from "./cache.js";
import { buildContextIntelligence } from "./pipeline.js";

let cleanup: (() => void) | undefined;
afterEach(() => {
  cleanup?.();
  cleanup = undefined;
});

describe("buildContextIntelligence", () => {
  it("composes every source into one ContextIntelligenceResult", () => {
    const repo = withTempRepo({
      "package.json": JSON.stringify({
        name: "demo",
        license: "MIT",
        devDependencies: { typescript: "^5.0.0" },
      }),
      "tsconfig.json": "{}",
      "CONTRIBUTING.md":
        "# Contributing\n\n## No unwrap in production\n\nHandle errors explicitly.\n",
      "README.md": "# Demo\n",
    });
    cleanup = repo.cleanup;

    const context = buildRepositoryContext(repo.root);
    const result = buildContextIntelligence(repo.root, context);

    const categories = new Set(result.items.map((i) => i.category));
    expect(categories.has("engineering-convention")).toBe(true);
    expect(categories.has("documentation")).toBe(true);
    expect(categories.has("capability")).toBe(true);
    expect(categories.has("project-metadata")).toBe(true);
    expect(result.rootDir).toBe(repo.root);
  });

  it("never produces a finding-shaped item — every item is inspectable as structured context, not a judgment", () => {
    const repo = withTempRepo({ "package.json": JSON.stringify({ name: "demo" }) });
    cleanup = repo.cleanup;
    const context = buildRepositoryContext(repo.root);
    const result = buildContextIntelligence(repo.root, context);

    for (const item of result.items) {
      expect(item).not.toHaveProperty("severity");
      expect(item).not.toHaveProperty("findingId");
    }
  });

  it("serves a cached result when nothing relevant changed", () => {
    const repo = withTempRepo({ "package.json": JSON.stringify({ name: "demo" }) });
    cleanup = repo.cleanup;
    const cache = createInMemoryContextCache();
    const context = buildRepositoryContext(repo.root);

    const first = buildContextIntelligence(repo.root, context, { cache });
    const second = buildContextIntelligence(repo.root, context, { cache });
    expect(second).toBe(first);
  });

  it("invalidates the cache when a workflow file is added", () => {
    const repo = withTempRepo({ "package.json": JSON.stringify({ name: "demo" }) });
    cleanup = repo.cleanup;
    const cache = createInMemoryContextCache();
    const context = buildRepositoryContext(repo.root);

    const first = buildContextIntelligence(repo.root, context, { cache });

    writeFileSync(join(repo.root, "CHANGELOG.md"), "# Changelog\n");
    const second = buildContextIntelligence(repo.root, context, { cache });

    expect(second).not.toBe(first);
    expect(second.items.some((i) => i.category === "workflow")).toBe(true);
  });

  it("invalidates the cache when the underlying RepositoryContext is regenerated", () => {
    const repo = withTempRepo({ "package.json": JSON.stringify({ name: "demo" }) });
    cleanup = repo.cleanup;
    const cache = createInMemoryContextCache();

    const contextA = buildRepositoryContext(repo.root);
    const first = buildContextIntelligence(repo.root, contextA, { cache });

    const contextB = { ...contextA, generatedAt: "2099-01-01T00:00:00.000Z" };
    const second = buildContextIntelligence(repo.root, contextB, { cache });

    expect(second).not.toBe(first);
  });
});
