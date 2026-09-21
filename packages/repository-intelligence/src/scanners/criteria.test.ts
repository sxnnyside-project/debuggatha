import { afterEach, describe, expect, it } from "bun:test";
import { readdirSync } from "node:fs";
import { withTempRepo } from "@debuggatha/testing";
import { scanCriteria } from "./criteria.js";

function scan(root: string) {
  return scanCriteria(root, readdirSync(root));
}

describe("scanCriteria", () => {
  let cleanup: (() => void) | undefined;

  afterEach(() => {
    cleanup?.();
    cleanup = undefined;
  });

  it("turns ESLint rules into structured objects, not raw text", () => {
    const repo = withTempRepo({
      ".eslintrc.json": JSON.stringify({ rules: { "no-console": "warn", eqeqeq: "error" } }),
    });
    cleanup = repo.cleanup;

    const { profile } = scan(repo.root);

    expect(profile.rules).toContainEqual({
      id: "eslint:no-console",
      description: 'ESLint rule "no-console" configured',
      source: { file: ".eslintrc.json", detail: '"warn"' },
    });
    expect(profile.sources).toContain(".eslintrc.json");
  });

  it("flattens Biome linter rules and formatter settings", () => {
    const repo = withTempRepo({
      "biome.json": JSON.stringify({
        linter: { rules: { recommended: true, style: { useConst: "error" } } },
        formatter: { indentStyle: "space", lineWidth: 100 },
      }),
    });
    cleanup = repo.cleanup;

    const { profile } = scan(repo.root);
    const ids = profile.rules.map((r) => r.id);

    expect(ids).toContain("biome:linter.recommended");
    expect(ids).toContain("biome:style.useConst");
    expect(ids).toContain("biome:formatter.indentStyle");
  });

  it("parses .editorconfig sections into per-section rules", () => {
    const repo = withTempRepo({
      ".editorconfig": "root = true\n\n[*.rs]\nindent_style = space\nindent_size = 4\n",
    });
    cleanup = repo.cleanup;

    const { profile } = scan(repo.root);

    expect(profile.rules).toContainEqual({
      id: "editorconfig:*.rs:indent_style",
      description: 'EditorConfig "indent_style" = "space" for *.rs',
      source: { file: ".editorconfig", detail: "[*.rs] indent_style = space" },
    });
  });

  it("extracts flat rustfmt.toml settings without a full TOML parser", () => {
    const repo = withTempRepo({
      "rustfmt.toml": 'max_width = 100\nedition = "2021"\n',
    });
    cleanup = repo.cleanup;

    const { profile } = scan(repo.root);
    const maxWidth = profile.rules.find((r) => r.id === "rustfmt:max_width");

    expect(maxWidth?.description).toBe('rustfmt "max_width" = 100');
  });

  it("records detekt.yml presence honestly without pretending to parse it", () => {
    const repo = withTempRepo({ "detekt.yml": "complexity:\n  active: true\n" });
    cleanup = repo.cleanup;

    const { profile } = scan(repo.root);

    expect(profile.rules).toContainEqual({
      id: "detekt:present",
      description:
        "detekt configuration present (contents not parsed — see repository-intelligence risks)",
      source: { file: "detekt.yml", detail: "detekt.yml present" },
    });
  });

  it("structures rule-shaped Markdown sections at heading granularity, without interpreting prose", () => {
    const repo = withTempRepo({
      "CONTRIBUTING.md":
        "# Contributing\n\n## No unwrap() in production code\n\nUse `?` instead.\n\n## Commit style\n\nUse Conventional Commits.\n",
    });
    cleanup = repo.cleanup;

    const { profile } = scan(repo.root);
    const descriptions = profile.rules.map((r) => r.description);

    expect(descriptions).toContain("No unwrap() in production code");
    expect(descriptions).toContain("Commit style");
  });

  it("returns no rules for a repo with no config or rule-shaped docs", () => {
    const repo = withTempRepo({ "src/index.ts": "export {};\n" });
    cleanup = repo.cleanup;

    const { profile } = scan(repo.root);

    expect(profile.rules).toHaveLength(0);
    expect(profile.sources).toHaveLength(0);
  });
});
