import { execFileSync, execSync } from "node:child_process";
import type { RepositoryProvider } from "../../provider.js";

/** Branch, tag, sha, or range (`a...b`); never starts with `-`, so it cannot be read as a git option. */
export const SAFE_REF = /^[\w@{}][\w./@{}^~:-]*$/;

export class LocalGitProvider implements RepositoryProvider {
  constructor(private readonly cwd: string) {}

  getRootDir(): string {
    return this.cwd;
  }

  async getDiff(base?: string): Promise<string | undefined> {
    if (base !== undefined && !SAFE_REF.test(base)) {
      throw new Error(`Unsafe git ref "${base}".`);
    }
    try {
      const diff = execFileSync("git", ["diff", base ?? "HEAD"], {
        cwd: this.cwd,
        encoding: "utf-8",
        stdio: ["pipe", "pipe", "ignore"],
      });
      return diff.trim() || undefined;
    } catch {
      return undefined;
    }
  }

  async getStatus(): Promise<string> {
    try {
      const status = execSync("git status --short", {
        cwd: this.cwd,
        encoding: "utf-8",
        stdio: ["pipe", "pipe", "ignore"],
      });
      return status.trim();
    } catch {
      return "";
    }
  }

  async getBranch(): Promise<string> {
    try {
      const branch = execSync("git rev-parse --abbrev-ref HEAD", {
        cwd: this.cwd,
        encoding: "utf-8",
        stdio: ["pipe", "pipe", "ignore"],
      });
      return branch.trim();
    } catch {
      return "";
    }
  }
}
