import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";
import type { GitContextItem } from "../types.js";

/**
 * Git Context (Epic 14): reads `.git`'s own plumbing files directly
 * (`HEAD`, `refs/remotes/origin/HEAD`) rather than shelling out to the
 * `git` binary — the same "no external process" discipline
 * `@debuggatha/repository-intelligence`'s `hasGit` check already uses
 * (`existsSync(.git)`, nothing more). "Do not perform semantic analysis
 * of commit history yet. Only expose structured context" (Epic 14) — no
 * commit log is read at all, only ref pointers.
 */
export function buildGitContextItems(rootDir: string): GitContextItem[] {
  const items: GitContextItem[] = [];
  const gitDir = join(rootDir, ".git");
  if (!existsSync(gitDir)) return items;

  const activeBranch = readSymbolicRef(join(gitDir, "HEAD"));
  if (activeBranch) {
    items.push({
      id: "git:active-branch",
      category: "git",
      confidence: "detected",
      evidence: [{ file: ".git/HEAD", detail: `HEAD points at refs/heads/${activeBranch}` }],
      source: ".git/HEAD",
      key: "active-branch",
      value: activeBranch,
    });
  }

  const defaultBranchRef = join(gitDir, "refs", "remotes", "origin", "HEAD");
  const defaultBranch = existsSync(defaultBranchRef)
    ? readSymbolicRef(defaultBranchRef, "refs/remotes/origin/")
    : undefined;
  if (defaultBranch) {
    items.push({
      id: "git:default-branch",
      category: "git",
      confidence: "detected",
      evidence: [
        {
          file: ".git/refs/remotes/origin/HEAD",
          detail: `Symbolic ref points at origin/${defaultBranch}`,
        },
      ],
      source: ".git/refs/remotes/origin/HEAD",
      key: "default-branch",
      value: defaultBranch,
    });
  }

  return items;
}

function readSymbolicRef(path: string, stripPrefix = "refs/heads/"): string | undefined {
  if (!existsSync(path)) return undefined;
  let content: string;
  try {
    content = readFileSync(path, "utf8").trim();
  } catch {
    return undefined;
  }
  const match = /^ref:\s*(\S+)$/.exec(content);
  const ref = match?.[1];
  if (!ref || !ref.startsWith(stripPrefix)) return undefined;
  return ref.slice(stripPrefix.length);
}
