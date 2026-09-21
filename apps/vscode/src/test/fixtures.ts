import { execFileSync } from "node:child_process";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { executeReview } from "@debuggatha/engine";

/** Source that the detected packs flag: `any`, `==`, `eval`, and a hardcoded secret. */
export const BAD_SOURCE =
  'var x: any = 1;\nif (x == 2) { eval("1"); }\nconst password = "hunter22222";\n';

const created: string[] = [];

export function makeRepo(source = BAD_SOURCE): { root: string; badFile: string } {
  const root = mkdtempSync(join(tmpdir(), "debuggatha-vscode-"));
  created.push(root);
  mkdirSync(join(root, "src"));
  writeFileSync(
    join(root, "package.json"),
    JSON.stringify({ name: "fixture", devDependencies: { typescript: "^5.0.0" } }),
  );
  const badFile = join(root, "src", "bad.ts");
  writeFileSync(badFile, source);
  return { root, badFile };
}

/** Runs a real workspace review so the repository has a ledger with genuine entries. */
export function seedLedger(root: string) {
  return executeReview({
    rootDir: root,
    scope: { kind: "workspace" },
    depth: "full",
    packIds: [],
    policyId: undefined,
    sourceName: "vscode-test",
  });
}

export function git(cwd: string, ...args: string[]): void {
  execFileSync(
    "git",
    ["-c", "user.name=t", "-c", "user.email=t@t", "-c", "commit.gpgsign=false", ...args],
    { cwd, stdio: "ignore" },
  );
}

export function removeRepos(): void {
  for (const dir of created.splice(0)) rmSync(dir, { recursive: true, force: true });
}
