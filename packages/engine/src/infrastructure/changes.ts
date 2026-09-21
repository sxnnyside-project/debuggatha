import { execFileSync } from "node:child_process";
import { statSync } from "node:fs";
import { join } from "node:path";
import { isIgnoredPath } from "../scan-scope/index.js";
import { SAFE_REF } from "./providers/git.js";

export interface ChangedFiles {
  /** Files that exist now and were added, modified, or are new and untracked, relative to the repository directory. */
  changed: string[];
  /** Files that existed at the base and are gone now. Reviewing them resolves whatever the ledger held for them. */
  deleted: string[];
  /** What the changes are measured against: a ref, or `null` in a repository with no commits yet. */
  base: string | null;
}

function git(cwd: string, args: string[]): string {
  return execFileSync("git", args, {
    cwd,
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
    maxBuffer: 256 * 1024 * 1024,
  });
}

/**
 * Files an agent (or a person) changed in the working tree: edits, additions,
 * deletions, and new files git does not track yet. A diff cannot say the last
 * two; this can, which is what lets a review notice that a file was fixed by
 * being removed. Paths a project does not own (dependencies, build output) are
 * left out.
 */
export function listChangedFiles(rootDir: string, base?: string): ChangedFiles {
  if (base !== undefined && !SAFE_REF.test(base)) throw new Error(`Unsafe git ref "${base}".`);

  try {
    git(rootDir, ["rev-parse", "--is-inside-work-tree"]);
  } catch {
    throw new Error(
      "This is not a git repository, so there is no record of what changed. Review specific files with review_files, or the whole repository with review_workspace.",
    );
  }

  let hasHead = true;
  try {
    git(rootDir, ["rev-parse", "--verify", "--quiet", "HEAD"]);
  } catch {
    hasHead = false;
  }
  const ref = base ?? (hasHead ? "HEAD" : null);

  const changed = new Set<string>();
  const deleted = new Set<string>();

  if (ref) {
    const parts = git(rootDir, [
      "diff",
      "--name-status",
      "-z",
      "--no-renames",
      "--relative",
      ref,
    ]).split("\0");
    for (let i = 0; i + 1 < parts.length; i += 2) {
      const status = parts[i] ?? "";
      const path = parts[i + 1] ?? "";
      if (!path) continue;
      (status.startsWith("D") ? deleted : changed).add(path);
    }
  } else {
    // No commits yet: everything staged is new.
    for (const path of git(rootDir, ["ls-files", "-z", "--cached"]).split("\0")) {
      if (path) changed.add(path);
    }
  }
  for (const path of git(rootDir, ["ls-files", "-z", "--others", "--exclude-standard"]).split(
    "\0",
  )) {
    if (path) changed.add(path);
  }

  const owned = (path: string) => !isIgnoredPath(rootDir, path);
  const exists = (path: string) => {
    try {
      return statSync(join(rootDir, path)).isFile();
    } catch {
      return false;
    }
  };

  return {
    changed: [...changed].filter(owned).filter(exists).sort(),
    // A file "deleted" relative to the base that is back on disk was restored, not deleted.
    deleted: [...deleted]
      .filter(owned)
      .filter((path) => !exists(path))
      .sort(),
    base: ref,
  };
}
