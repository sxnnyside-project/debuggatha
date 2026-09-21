import { afterEach, describe, expect, it } from "bun:test";
import { writeFileSync } from "node:fs";
import { join } from "node:path";
import { withTempRepo } from "@debuggatha/testing";
import { createInMemoryCache } from "./cache/index.js";
import { buildRepositoryContext } from "./snapshot.js";

// A Tauri + Rust + React fixture, matching the walkthrough example in
// CLAUDE.md — the same repo shape used to illustrate the product thesis
// should also prove the implementation actually works against it.
const TAURI_REACT_FIXTURE = {
  "Cargo.toml":
    '[package]\nname = "demo"\nrust-version = "1.75"\n\n[dependencies]\ntauri = "2.1"\n',
  "package.json": JSON.stringify({
    name: "demo",
    packageManager: "bun@1.2.20",
    dependencies: { react: "^18.2.0", "@tauri-apps/api": "^2.1.0" },
  }),
  "bun.lock": "",
  "README.md": "# Demo\n\nA Tauri desktop app with a React frontend.\n",
  "ARCHITECTURE.md": "# Architecture\n\n## Layered core/adapters separation\n\nCore has no IO.\n",
  "CONTRIBUTING.md": "# Contributing\n\n## No unwrap() in production code\n\nUse `?` instead.\n",
};

describe("buildRepositoryContext", () => {
  let cleanup: (() => void) | undefined;

  afterEach(() => {
    cleanup?.();
    cleanup = undefined;
  });

  it("assembles a complete RepositoryContext from a realistic Tauri + Rust + React repo", () => {
    const repo = withTempRepo(TAURI_REACT_FIXTURE);
    cleanup = repo.cleanup;

    const context = buildRepositoryContext(repo.root);

    expect(context.stack.languages.map((s) => s.value).sort()).toEqual([
      "JavaScript/TypeScript",
      "Rust",
    ]);
    expect(context.stack.frameworks.map((s) => s.value).sort()).toEqual(["React", "Tauri"]);
    expect(context.stack.platformTargets).toEqual(["desktop", "web"]);
    expect(context.stack.packageManagers.map((s) => s.value)).toContain("Bun");
    expect(context.stack.runtimes.map((s) => s.value)).toContain("Bun");

    expect(context.dependencies.declaredVersions.tauri).toBe("2.1");
    expect(context.dependencies.declaredVersions.react).toBe("^18.2.0");
    expect(context.dependencies.runtimeConstraints.rust).toBe("1.75");

    const docFiles = context.documentation.sources.map((s) => s.file).sort();
    expect(docFiles).toEqual(["ARCHITECTURE.md", "CONTRIBUTING.md", "README.md"]);

    const ruleDescriptions = context.criteria.rules.map((r) => r.description);
    expect(ruleDescriptions).toContain("Layered core/adapters separation");
    expect(ruleDescriptions).toContain("No unwrap() in production code");

    expect(context.understanding.confidence).toBe("high");
    expect(context.understanding.openQuestions).toHaveLength(0);
  });

  it("raises open questions and lowers confidence for a repo Repository Intelligence can't confidently explain", () => {
    const repo = withTempRepo({ "some-data.csv": "a,b,c\n" });
    cleanup = repo.cleanup;

    const context = buildRepositoryContext(repo.root);

    expect(context.understanding.confidence).toBe("low");
    expect(context.understanding.openQuestions.length).toBeGreaterThan(0);
  });

  it("is deeply frozen — nothing downstream can mutate a shared snapshot", () => {
    const repo = withTempRepo({ "README.md": "# Demo\n" });
    cleanup = repo.cleanup;

    const context = buildRepositoryContext(repo.root);

    expect(Object.isFrozen(context)).toBe(true);
    expect(Object.isFrozen(context.stack)).toBe(true);
    expect(() => {
      // @ts-expect-error — intentionally violating readonly to prove runtime immutability
      context.rootDir = "/tampered";
    }).toThrow();
  });

  it("serves an unchanged repository from cache instead of rescanning", () => {
    const repo = withTempRepo(TAURI_REACT_FIXTURE);
    cleanup = repo.cleanup;
    const cache = createInMemoryCache();

    const first = buildRepositoryContext(repo.root, { cache });
    const second = buildRepositoryContext(repo.root, { cache });

    expect(second).toBe(first);
  });

  it("invalidates the cache when a file it read changes", () => {
    const repo = withTempRepo(TAURI_REACT_FIXTURE);
    cleanup = repo.cleanup;
    const cache = createInMemoryCache();

    const first = buildRepositoryContext(repo.root, { cache });
    writeFileSync(join(repo.root, "README.md"), "# Demo\n\nRewritten.\n");
    const second = buildRepositoryContext(repo.root, { cache });

    expect(second).not.toBe(first);
    expect(second.documentation.sources.find((s) => s.file === "README.md")?.wordCount).not.toBe(
      first.documentation.sources.find((s) => s.file === "README.md")?.wordCount,
    );
  });

  it("invalidates the cache when a new manifest appears, even though no previously-tracked file changed", () => {
    const repo = withTempRepo({ "README.md": "# Demo\n" });
    cleanup = repo.cleanup;
    const cache = createInMemoryCache();

    const first = buildRepositoryContext(repo.root, { cache });
    expect(first.stack.languages).toHaveLength(0);

    writeFileSync(join(repo.root, "Cargo.toml"), '[package]\nname = "demo"\n');
    const second = buildRepositoryContext(repo.root, { cache });

    expect(second).not.toBe(first);
    expect(second.stack.languages.map((s) => s.value)).toContain("Rust");
  });
});
