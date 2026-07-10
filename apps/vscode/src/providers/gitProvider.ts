import { execSync } from "node:child_process";
import type { RepositoryProvider } from "@debuggatha/core";
import * as vscode from "vscode";

export class VSCodeGitProvider implements RepositoryProvider {
  getRootDir(): string {
    return vscode.workspace.workspaceFolders?.[0]?.uri.fsPath || "";
  }

  async getDiff(cwd?: string): Promise<string | undefined> {
    try {
      return execSync("git diff HEAD", { cwd, encoding: "utf-8" });
    } catch (e: any) {
      return "";
    }
  }

  getUncommittedFiles(cwd: string): string[] {
    try {
      const output = execSync("git ls-files --others --modified --exclude-standard", {
        cwd,
        encoding: "utf-8",
      });
      return output.split("\n").filter(Boolean);
    } catch (e: any) {
      return [];
    }
  }

  readFileAtCommit(cwd: string, filepath: string, commitHash: string): string {
    try {
      return execSync(`git show ${commitHash}:${filepath}`, { cwd, encoding: "utf-8" });
    } catch (e: any) {
      return "";
    }
  }

  async getStatus(): Promise<string> {
    try {
      const rootDir = this.getRootDir();
      const status = execSync("git status --short", {
        cwd: rootDir,
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
      const rootDir = this.getRootDir();
      const branch = execSync("git rev-parse --abbrev-ref HEAD", {
        cwd: rootDir,
        encoding: "utf-8",
        stdio: ["pipe", "pipe", "ignore"],
      });
      return branch.trim();
    } catch {
      return "";
    }
  }
}
