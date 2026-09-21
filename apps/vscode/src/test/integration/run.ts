import { chmodSync, mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { runTests } from "@vscode/test-electron";

// The oldest VS Code the manifest claims to support (`engines.vscode`), unless a run asks for another
// (`VSCODE_TEST_VERSION=stable`).
const VSCODE_VERSION = process.env.VSCODE_TEST_VERSION ?? "1.90.2";

/** A small TypeScript repository with lines the detected packs flag, and a Python file for Ruff. */
function makeWorkspace(): string {
  const root = mkdtempSync(join(tmpdir(), "debuggatha-itest-"));
  mkdirSync(join(root, "src"));
  writeFileSync(
    join(root, "package.json"),
    JSON.stringify({ name: "fixture", devDependencies: { typescript: "^5.0.0" } }),
  );
  writeFileSync(
    join(root, "src", "bad.ts"),
    'var x: any = 1;\nif (x == 2) { eval("1"); }\nconst password = "hunter22222";\n',
  );
  writeFileSync(join(root, "app.py"), "import os\n");
  return root;
}

/**
 * A stand-in for Ruff, installed where a Python project keeps its own (`.venv/bin/ruff`): VS Code
 * on macOS replaces the PATH with the login shell's, so a tool on the PATH would not be found,
 * and a project's own install is what a review should prefer anyway. It answers at once with no
 * findings, records its pid so a test can see whether it ran, and, when the workspace has a
 * `.slow-ruff` file, keeps running so a test can cancel a review that is waiting on an analyzer.
 */
function fakeRuff(workspace: string): { pidFile: string } {
  const bin = join(workspace, ".venv", "bin");
  mkdirSync(bin, { recursive: true });
  const pidFile = join(mkdtempSync(join(tmpdir(), "debuggatha-itest-pid-")), "ruff.pid");
  writeFileSync(
    join(bin, "ruff"),
    `#!/bin/sh
if [ "$1" = "--version" ]; then echo "ruff 0.15.1"; exit 0; fi
echo "$$" > "${pidFile}"
if [ -f "$PWD/.slow-ruff" ]; then sleep 60; fi
echo '[]'
`,
  );
  chmodSync(join(bin, "ruff"), 0o755);
  return { pidFile };
}

async function main() {
  const workspace = makeWorkspace();
  const userData = mkdtempSync(join(tmpdir(), "debuggatha-itest-user-"));
  const extensionsDir = mkdtempSync(join(tmpdir(), "debuggatha-itest-ext-"));
  const { pidFile } = fakeRuff(workspace);

  await runTests({
    version: VSCODE_VERSION,
    extensionDevelopmentPath: resolve(__dirname, ".."),
    extensionTestsPath: resolve(__dirname, "suite.js"),
    extensionTestsEnv: {
      DEBUGGATHA_TEST_WORKSPACE: workspace,
      DEBUGGATHA_TEST_RUFF_PID: pidFile,
    },
    launchArgs: [
      workspace,
      "--disable-extensions",
      "--disable-workspace-trust",
      "--skip-welcome",
      "--skip-release-notes",
      `--user-data-dir=${userData}`,
      `--extensions-dir=${extensionsDir}`,
    ],
  });
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
