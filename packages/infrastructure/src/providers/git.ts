import { execSync } from "node:child_process";
import type { RepositoryProvider } from "@debuggatha/core";

export class LocalGitProvider implements RepositoryProvider {
  constructor(private readonly cwd: string) {}

  getRootDir(): string {
    return this.cwd;
  }

  async getDiff(base?: string): Promise<string | undefined> {
    try {
      const command = base ? `git diff ${base}` : "git diff HEAD";
      const diff = execSync(command, {
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
