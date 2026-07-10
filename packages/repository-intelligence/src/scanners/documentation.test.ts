import { readdirSync } from "node:fs";
import { withTempRepo } from "@debuggatha/testing";
import { afterEach, describe, expect, it } from "vitest";
import { scanDocumentation } from "./documentation.js";

function scan(root: string) {
  return scanDocumentation(root, readdirSync(root));
}

describe("scanDocumentation", () => {
  let cleanup: (() => void) | undefined;

  afterEach(() => {
    cleanup?.();
    cleanup = undefined;
  });

  it("discovers root-level documentation and extracts the first heading as title", () => {
    const repo = withTempRepo({
      "README.md": "# Demo\n\nA short description with six words.\n",
    });
    cleanup = repo.cleanup;

    const { profile } = scan(repo.root);

    const readme = profile.sources.find((s) => s.file === "README.md");
    expect(readme).toMatchObject({ kind: "readme", title: "Demo" });
    expect(readme?.wordCount).toBeGreaterThan(0);
  });

  it("classifies CLAUDE.md, CONTRIBUTING.md, and ARCHITECTURE.md correctly", () => {
    const repo = withTempRepo({
      "CLAUDE.md": "# Memory\n",
      "CONTRIBUTING.md": "# Contributing\n",
      "ARCHITECTURE.md": "# Architecture\n",
    });
    cleanup = repo.cleanup;

    const { profile } = scan(repo.root);
    const kinds = new Map(profile.sources.map((s) => [s.file, s.kind]));

    expect(kinds.get("CLAUDE.md")).toBe("claude-memory");
    expect(kinds.get("CONTRIBUTING.md")).toBe("contributing");
    expect(kinds.get("ARCHITECTURE.md")).toBe("architecture");
  });

  it("recurses into docs/adr to find nested ADRs", () => {
    const repo = withTempRepo({
      "docs/adr/0001-use-postgres.md": "# 0001: Use Postgres\n",
    });
    cleanup = repo.cleanup;

    const { profile } = scan(repo.root);

    const adr = profile.sources.find((s) => s.file === "docs/adr/0001-use-postgres.md");
    expect(adr?.kind).toBe("adr");
  });

  it("skips ignored directories like node_modules under docs", () => {
    const repo = withTempRepo({
      "docs/guide.md": "# Guide\n",
      "docs/node_modules/should-not-be-read.md": "# Ignore me\n",
    });
    cleanup = repo.cleanup;

    const { profile } = scan(repo.root);

    expect(profile.sources.some((s) => s.file.includes("node_modules"))).toBe(false);
    expect(profile.sources.some((s) => s.file === "docs/guide.md")).toBe(true);
  });

  it("produces a factual, non-empty summary and an honest empty one when nothing is found", () => {
    const withDocs = withTempRepo({ "README.md": "# Demo\n" });
    const withoutDocs = withTempRepo({ "src/index.ts": "export {};\n" });

    const withDocsResult = scan(withDocs.root);
    const withoutDocsResult = scan(withoutDocs.root);
    withDocs.cleanup();
    withoutDocs.cleanup();

    expect(withDocsResult.profile.summary).toContain("1 documentation source");
    expect(withoutDocsResult.profile.summary).toBe("No documentation sources found.");
  });

  it("tracks every directory it listed, for cache fingerprinting", () => {
    const repo = withTempRepo({ "docs/adr/0001-x.md": "# X\n" });
    cleanup = repo.cleanup;

    const result = scan(repo.root);

    const listedDirs = result.dirsListed.map((d) => d.dir);
    expect(listedDirs.some((d) => d.endsWith("docs"))).toBe(true);
    expect(listedDirs.some((d) => d.endsWith("docs/adr") || d.endsWith("docs\\adr"))).toBe(true);
  });
});
