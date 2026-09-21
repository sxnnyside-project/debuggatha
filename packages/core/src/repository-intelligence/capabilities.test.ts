import { afterEach, describe, expect, it } from "bun:test";
import { readdirSync } from "node:fs";
import { withTempRepo } from "../testing/index.js";
import { deriveCapabilities } from "./capabilities.js";
import { scanStack } from "./scanners/stack.js";

function capsFor(root: string) {
  const rootEntries = readdirSync(root);
  const stackScan = scanStack(root, rootEntries);
  return deriveCapabilities(root, rootEntries, stackScan);
}

describe("deriveCapabilities", () => {
  let cleanup: (() => void) | undefined;

  afterEach(() => {
    cleanup?.();
    cleanup = undefined;
  });

  it("emits javascript but not typescript for a plain JS repo", () => {
    const repo = withTempRepo({ "package.json": JSON.stringify({ name: "demo" }) });
    cleanup = repo.cleanup;

    const caps = capsFor(repo.root);
    expect(caps.some((c) => c.id === "javascript")).toBe(true);
    expect(caps.some((c) => c.id === "typescript")).toBe(false);
  });

  it("emits both javascript and typescript, additively, for a TS repo", () => {
    const repo = withTempRepo({
      "package.json": JSON.stringify({ name: "demo", devDependencies: { typescript: "^5.0.0" } }),
      "tsconfig.json": "{}",
    });
    cleanup = repo.cleanup;

    const caps = capsFor(repo.root);
    const js = caps.find((c) => c.id === "javascript");
    const ts = caps.find((c) => c.id === "typescript");
    expect(js).toBeDefined();
    expect(ts).toBeDefined();
    expect(ts?.confidence).toBe("high");
    expect(js?.kind).toBe("language");
    expect(ts?.kind).toBe("language");
  });

  it("still emits typescript at medium confidence from a devDependency alone, without tsconfig.json", () => {
    const repo = withTempRepo({
      "package.json": JSON.stringify({ name: "demo", devDependencies: { typescript: "^5.0.0" } }),
    });
    cleanup = repo.cleanup;

    const ts = capsFor(repo.root).find((c) => c.id === "typescript");
    expect(ts?.confidence).toBe("medium");
  });

  it("resolves multiple frameworks additively, never picking just one", () => {
    const repo = withTempRepo({
      "package.json": JSON.stringify({
        name: "demo",
        dependencies: { react: "^18.0.0", express: "^4.0.0", astro: "^4.0.0" },
      }),
    });
    cleanup = repo.cleanup;

    const ids = capsFor(repo.root)
      .filter((c) => c.kind === "framework")
      .map((c) => c.id)
      .sort();
    expect(ids).toEqual(["astro", "express", "react"]);
  });

  it("detects a polyglot repo's languages independently (Rust + JS/TS at once)", () => {
    const repo = withTempRepo({
      "package.json": JSON.stringify({ name: "demo", devDependencies: { typescript: "^5.0.0" } }),
      "Cargo.toml": '[package]\nname = "demo"\n',
      "tsconfig.json": "{}",
    });
    cleanup = repo.cleanup;

    const languageIds = capsFor(repo.root)
      .filter((c) => c.kind === "language")
      .map((c) => c.id)
      .sort();
    expect(languageIds).toEqual(["javascript", "rust", "typescript"]);
  });

  it("detects a VS Code extension platform from engines.vscode", () => {
    const repo = withTempRepo({
      "package.json": JSON.stringify({ name: "demo", engines: { vscode: "^1.90.0" } }),
    });
    cleanup = repo.cleanup;

    expect(
      capsFor(repo.root).some((c) => c.id === "vscodeextension" && c.kind === "platform"),
    ).toBe(true);
  });

  it("detects PHP and Laravel from composer.json", () => {
    const repo = withTempRepo({
      "composer.json": JSON.stringify({ require: { "laravel/framework": "^11.0" } }),
    });
    cleanup = repo.cleanup;

    const caps = capsFor(repo.root);
    expect(caps.some((c) => c.id === "php" && c.kind === "language")).toBe(true);
    expect(caps.some((c) => c.id === "laravel" && c.kind === "framework")).toBe(true);
  });

  it("detects PHP without Laravel when composer.json has no laravel/framework dependency", () => {
    const repo = withTempRepo({
      "composer.json": JSON.stringify({ require: { "monolog/monolog": "^3.0" } }),
    });
    cleanup = repo.cleanup;

    const caps = capsFor(repo.root);
    expect(caps.some((c) => c.id === "php")).toBe(true);
    expect(caps.some((c) => c.id === "laravel")).toBe(false);
  });

  it("detects monorepo/package-based characteristics from a packages/ directory", () => {
    const repo = withTempRepo({
      "package.json": JSON.stringify({ name: "demo" }),
      "packages/a/package.json": JSON.stringify({ name: "a" }),
    });
    cleanup = repo.cleanup;

    const ids = capsFor(repo.root)
      .filter((c) => c.kind === "characteristic")
      .map((c) => c.id)
      .sort();
    expect(ids).toContain("monorepo");
    expect(ids).toContain("packagebased");
  });

  it("detects a layered characteristic from src/domain + src/adapters", () => {
    const repo = withTempRepo({
      "package.json": JSON.stringify({ name: "demo" }),
      "src/domain/user.ts": "export class User {}\n",
      "src/adapters/db.ts": "export class Db {}\n",
    });
    cleanup = repo.cleanup;

    expect(capsFor(repo.root).some((c) => c.id === "layered")).toBe(true);
  });

  it("detects tooling from config files (biome, eslint, vitest)", () => {
    const repo = withTempRepo({
      "package.json": JSON.stringify({ name: "demo" }),
      "biome.json": "{}",
      "vitest.config.ts": "export default {}",
    });
    cleanup = repo.cleanup;

    const toolingIds = capsFor(repo.root)
      .filter((c) => c.kind === "tooling")
      .map((c) => c.id);
    expect(toolingIds).toContain("biome");
    expect(toolingIds).toContain("vitest");
  });

  it("returns no capabilities for an ambiguous repo with no recognizable manifest", () => {
    const repo = withTempRepo({ "README.md": "# demo\n" });
    cleanup = repo.cleanup;

    expect(capsFor(repo.root)).toEqual([]);
  });

  it("every capability carries evidence — never a bare claim", () => {
    const repo = withTempRepo({ "package.json": JSON.stringify({ name: "demo" }) });
    cleanup = repo.cleanup;

    for (const cap of capsFor(repo.root)) {
      expect(cap.evidence.length).toBeGreaterThan(0);
    }
  });
});
