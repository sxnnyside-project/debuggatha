import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { withTempRepo } from "@debuggatha/testing";
import { afterEach, describe, expect, it } from "vitest";
import { buildGitContextItems } from "./git.js";

let cleanup: (() => void) | undefined;
afterEach(() => {
  cleanup?.();
  cleanup = undefined;
});

describe("buildGitContextItems", () => {
  it("reads the active branch from .git/HEAD", () => {
    const repo = withTempRepo({ "package.json": "{}" });
    cleanup = repo.cleanup;
    mkdirSync(join(repo.root, ".git"), { recursive: true });
    writeFileSync(join(repo.root, ".git", "HEAD"), "ref: refs/heads/feature/x\n");

    const items = buildGitContextItems(repo.root);
    const active = items.find((i) => i.key === "active-branch");
    expect(active?.value).toBe("feature/x");
    expect(active?.confidence).toBe("detected");
  });

  it("reads the default branch from refs/remotes/origin/HEAD when present", () => {
    const repo = withTempRepo({ "package.json": "{}" });
    cleanup = repo.cleanup;
    mkdirSync(join(repo.root, ".git", "refs", "remotes", "origin"), { recursive: true });
    writeFileSync(join(repo.root, ".git", "HEAD"), "ref: refs/heads/main\n");
    writeFileSync(
      join(repo.root, ".git", "refs", "remotes", "origin", "HEAD"),
      "ref: refs/remotes/origin/develop\n",
    );

    const items = buildGitContextItems(repo.root);
    expect(items.find((i) => i.key === "default-branch")?.value).toBe("develop");
  });

  it("returns no items for a repo with no .git directory", () => {
    const repo = withTempRepo({ "package.json": "{}" });
    cleanup = repo.cleanup;
    expect(buildGitContextItems(repo.root)).toEqual([]);
  });

  it("returns only the active branch when there is no remote HEAD", () => {
    const repo = withTempRepo({ "package.json": "{}" });
    cleanup = repo.cleanup;
    mkdirSync(join(repo.root, ".git"), { recursive: true });
    writeFileSync(join(repo.root, ".git", "HEAD"), "ref: refs/heads/main\n");

    const items = buildGitContextItems(repo.root);
    expect(items).toHaveLength(1);
    expect(items[0]?.key).toBe("active-branch");
  });
});
