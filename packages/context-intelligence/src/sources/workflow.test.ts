import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { withTempRepo } from "@debuggatha/testing";
import { afterEach, describe, expect, it } from "vitest";
import { detectWorkflow } from "./workflow.js";

let cleanup: (() => void) | undefined;
afterEach(() => {
  cleanup?.();
  cleanup = undefined;
});

describe("detectWorkflow", () => {
  it("detects a CI workflow and extracts its triggers", () => {
    const repo = withTempRepo({
      ".github/workflows/ci.yml":
        "name: CI\non:\n  push:\n  pull_request:\njobs:\n  test:\n    runs-on: ubuntu-latest\n",
    });
    cleanup = repo.cleanup;

    const items = detectWorkflow(repo.root);
    const ci = items.find((i) => i.workflowKind === "ci");
    expect(ci).toBeDefined();
    expect(ci?.confidence).toBe("detected");
    expect(ci?.detail).toContain("push");
    expect(ci?.detail).toContain("pull_request");
  });

  it("detects release/versioning strategy from a CHANGELOG.md", () => {
    const repo = withTempRepo({ "CHANGELOG.md": "# Changelog\n" });
    cleanup = repo.cleanup;
    const items = detectWorkflow(repo.root);
    expect(items.some((i) => i.workflowKind === "release")).toBe(true);
  });

  it("detects a commit message convention from commitlint config", () => {
    const repo = withTempRepo({ "commitlint.config.js": "module.exports = {};\n" });
    cleanup = repo.cleanup;
    const items = detectWorkflow(repo.root);
    expect(items.some((i) => i.workflowKind === "commit-convention")).toBe(true);
  });

  it("detects a branching convention from release/hotfix/feature-prefixed local branches", () => {
    const repo = withTempRepo({ "package.json": "{}" });
    cleanup = repo.cleanup;
    mkdirSync(join(repo.root, ".git", "refs", "heads", "feature"), { recursive: true });
    writeFileSync(join(repo.root, ".git", "refs", "heads", "main"), "abc123\n");
    writeFileSync(join(repo.root, ".git", "refs", "heads", "feature", "x"), "def456\n");

    const items = detectWorkflow(repo.root);
    expect(items.some((i) => i.workflowKind === "branching-convention")).toBe(true);
  });

  it("returns no items for a repo with none of these markers", () => {
    const repo = withTempRepo({ "package.json": "{}" });
    cleanup = repo.cleanup;
    expect(detectWorkflow(repo.root)).toEqual([]);
  });
});
