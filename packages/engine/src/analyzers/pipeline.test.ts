import { afterEach, beforeEach, describe, expect, test } from "bun:test";
import {
  chmodSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  symlinkSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { delimiter, join } from "node:path";
import { ruleIdFromEvidence } from "@debuggatha/core";
import { executeReview, type ReviewPipelineInput } from "../pipeline.js";
import { inventoryAnalyzers } from "./index.js";
import type { AnalyzerOptions } from "./types.js";

/*
 * The runner against stand-in executables on a controlled PATH, so what it does
 * with a tool (find it, gate it, stop it, survive it) is tested without the
 * real tools. What the real tools print is covered by `parsers.test.ts`, and
 * Biome, which this repository has installed, runs for real at the end.
 */

let repo: string;
let bin: string;
let savedPath: string | undefined;

beforeEach(() => {
  repo = mkdtempSync(join(tmpdir(), "debuggatha-analyzers-repo-"));
  bin = mkdtempSync(join(tmpdir(), "debuggatha-analyzers-bin-"));
  savedPath = process.env.PATH;
  process.env.PATH = [bin, "/usr/bin", "/bin"].join(delimiter);
  writeFileSync(join(repo, "package.json"), JSON.stringify({ name: "fixture" }));
});

afterEach(() => {
  process.env.PATH = savedPath;
  rmSync(repo, { recursive: true, force: true });
  rmSync(bin, { recursive: true, force: true });
});

function install(name: string, options: { stdout?: string; exit?: number; body?: string } = {}) {
  writeFileSync(join(bin, `${name}.stdout`), options.stdout ?? "");
  writeFileSync(
    join(bin, name),
    `#!/bin/sh
if [ "$1" = "--version" ] || [ "$1" = "version" ]; then echo "9.9.9"; exit 0; fi
${options.body ?? ""}
cat "${join(bin, `${name}.stdout`)}"
exit ${options.exit ?? 0}
`,
  );
  chmodSync(join(bin, name), 0o755);
}

const review = (
  analyzers: AnalyzerOptions | undefined,
  overrides: Partial<ReviewPipelineInput> = {},
) =>
  executeReview({
    rootDir: repo,
    scope: { kind: "workspace" },
    depth: "full",
    packIds: [],
    policyId: undefined,
    sourceName: "test",
    ...(analyzers ? { analyzers } : {}),
    ...overrides,
  });

const ruffReport = (...codes: [code: string, row: number][]) =>
  JSON.stringify(
    codes.map(([code, row]) => ({
      code,
      message: `${code} message`,
      filename: join(repo, "app.py"),
      location: { row, column: 1 },
      end_location: { row, column: 5 },
      url: `https://docs.astral.sh/ruff/rules/${code}`,
    })),
  );

describe("which analyzers run", () => {
  test("none run unless a review asks for them", () => {
    writeFileSync(join(repo, "app.py"), "import os\n");
    install("ruff", { stdout: ruffReport(["F401", 1]) });
    const { analyzers, result } = review(undefined);
    expect(analyzers).toEqual([]);
    expect(result.findings.some((f) => ruleIdFromEvidence(f.evidence)?.startsWith("ruff:"))).toBe(
      false,
    );
  });

  test("a safe analyzer that is installed and applies runs by itself, and says what it is", () => {
    writeFileSync(join(repo, "app.py"), "import os\n");
    install("ruff", { stdout: ruffReport(["F401", 1]) });
    const { analyzers, result } = review({ mode: "auto" });

    expect(analyzers.find((run) => run.id === "ruff")).toMatchObject({
      status: "ran",
      findings: 1,
      version: "9.9.9",
      license: "MIT",
    });
    const finding = result.findings.find((f) => ruleIdFromEvidence(f.evidence) === "ruff:F401");
    expect(finding?.locations[0]).toMatchObject({ file: "app.py", lines: { start: 1, end: 1 } });
    expect(finding?.evidence).toContainEqual({
      kind: "external-analyzer",
      tool: "ruff",
      ruleId: "F401",
      version: "9.9.9",
      license: "MIT",
      url: "https://docs.astral.sh/ruff/rules/F401",
    });
    expect(finding?.evidence).toContainEqual({
      kind: "code",
      file: "app.py",
      lines: { start: 1, end: 1 },
      excerpt: "import os",
    });
  });

  test("a tool that is not installed is reported, not an error", () => {
    writeFileSync(join(repo, "app.py"), "import os\n");
    const { analyzers } = review({ mode: "auto" });
    expect(analyzers.find((run) => run.id === "ruff")).toMatchObject({
      status: "skipped",
      reason: "not installed",
    });
  });

  test("a tool the repository has no use for is left alone", () => {
    install("ruff", { stdout: "[]" });
    const { analyzers } = review({ mode: "auto" });
    expect(analyzers.find((run) => run.id === "ruff")?.status).toBe("skipped");
  });

  test("a tool that runs project code needs to be enabled by name", () => {
    writeFileSync(join(repo, "eslint.config.js"), "export default [];\n");
    writeFileSync(join(repo, "app.js"), "var a = 1;\n");
    install("eslint", { stdout: "[]" });

    const off = review({ mode: "auto" }).analyzers.find((run) => run.id === "eslint");
    expect(off?.status).toBe("skipped");
    expect(off?.reason).toContain('enable "eslint" explicitly');

    const on = review({ mode: "auto", enable: ["eslint"] }).analyzers.find(
      (run) => run.id === "eslint",
    );
    expect(on?.status).toBe("ran");
  });

  test("mode off runs nothing", () => {
    writeFileSync(join(repo, "app.py"), "import os\n");
    install("ruff", { stdout: ruffReport(["F401", 1]) });
    expect(review({ mode: "off" }).analyzers).toEqual([]);
  });

  test("the inventory shows what is installed and what would run", () => {
    writeFileSync(join(repo, "app.py"), "import os\n");
    writeFileSync(join(repo, "package-lock.json"), "{}");
    install("ruff");
    install("osv-scanner");
    const entries = Object.fromEntries(inventoryAnalyzers(repo).map((entry) => [entry.id, entry]));

    expect(entries.ruff).toMatchObject({ installed: true, version: "9.9.9", willRun: true });
    expect(entries["osv-scanner"]).toMatchObject({
      installed: true,
      willRun: false,
      trust: "network",
    });
    expect(entries["osv-scanner"]?.reason).toContain("sends package names");
    expect(entries.ktlint).toMatchObject({ installed: false, willRun: false });
  });
});

describe("when a tool misbehaves", () => {
  test("a failing tool does not fail the review, and the built-in findings survive", () => {
    writeFileSync(join(repo, "app.py"), "import os\n");
    writeFileSync(join(repo, "a.js"), "var x = 1;\n");
    install("ruff", { stdout: "", exit: 3, body: "echo 'bad config' >&2" });

    const { analyzers, result } = review({ mode: "auto" });
    expect(analyzers.find((run) => run.id === "ruff")).toMatchObject({
      status: "failed",
      reason: "exited with code 3: bad config",
    });
    expect(result.findings.some((f) => ruleIdFromEvidence(f.evidence) === "no-var")).toBe(true);
  });

  test("output that is not the tool's format is a failure, not a crash", () => {
    writeFileSync(join(repo, "app.py"), "import os\n");
    install("ruff", { stdout: "this is not json" });
    const run = review({ mode: "auto" }).analyzers.find((item) => item.id === "ruff");
    expect(run?.status).toBe("failed");
    expect(run?.reason).toContain("output not understood");
  });

  test("a tool that hangs is stopped", () => {
    writeFileSync(join(repo, "app.py"), "import os\n");
    install("ruff", { body: "sleep 10" });
    const started = Date.now();
    const run = review({ mode: "auto", timeoutSeconds: 0.3 }).analyzers.find(
      (item) => item.id === "ruff",
    );
    expect(run).toMatchObject({ status: "failed", reason: "timed out after 0.3s" });
    expect(Date.now() - started).toBeLessThan(5_000);
  });

  test("file names reach the tool as plain arguments, never through a shell", () => {
    const hostile = "a$(touch pwned).py";
    writeFileSync(join(repo, hostile), "import os\n");
    const log = join(bin, "argv.log");
    install("ruff", { stdout: "[]", body: `printf '%s\\n' "$@" >> "${log}"` });

    review({ mode: "auto" }, { scope: { kind: "files", paths: [hostile] } });
    expect(readFileSync(log, "utf8").split("\n")).toContain(hostile);
    expect(existsSync(join(repo, "pwned"))).toBe(false);
  });
});

describe("merging with the built-in detectors", () => {
  test("a secret scanner answers for secrets, and the secret is never in the report", () => {
    const secret = "sk_live_1234567890abcdef";
    writeFileSync(join(repo, "config.js"), `const apiKey = "${secret}";\n`);
    const withoutScanner = review(undefined, { packIds: ["debuggatha/release"] });
    expect(
      withoutScanner.result.findings.some(
        (f) => ruleIdFromEvidence(f.evidence) === "no-hardcoded-secrets",
      ),
    ).toBe(true);

    install("gitleaks", {
      stdout: JSON.stringify({
        version: "2.1.0",
        runs: [
          {
            tool: { driver: { name: "gitleaks", semanticVersion: "v8.30.1" } },
            results: [
              {
                ruleId: "generic-api-key",
                message: { text: "generic-api-key has detected secret for file config.js." },
                locations: [
                  {
                    physicalLocation: {
                      artifactLocation: { uri: "config.js" },
                      region: {
                        startLine: 1,
                        startColumn: 18,
                        endLine: 1,
                        endColumn: 38,
                        snippet: { text: "REDACTED" },
                      },
                    },
                  },
                ],
              },
            ],
          },
        ],
      }),
    });
    const { result, analyzers } = review({ mode: "auto" }, { packIds: ["debuggatha/release"] });

    expect(analyzers.find((run) => run.id === "gitleaks")?.status).toBe("ran");
    const rules = result.findings.map((f) => ruleIdFromEvidence(f.evidence));
    expect(rules).toContain("gitleaks:generic-api-key");
    expect(rules).not.toContain("no-hardcoded-secrets");
    expect(JSON.stringify(result)).not.toContain(secret);
    expect(readFileSync(join(repo, ".debuggatha", "ledger.json"), "utf8")).not.toContain(secret);
  });

  test("a linter's finding replaces the built-in one for the same rule and line, not the others", () => {
    writeFileSync(join(repo, "eslint.config.js"), "export default [];\n");
    writeFileSync(join(repo, "a.js"), "var x = 1;\nvar y = 2;\n");
    const eslintReport = JSON.stringify([
      {
        filePath: join(repo, "a.js"),
        messages: [
          {
            ruleId: "no-var",
            severity: 2,
            message: "Unexpected var, use let or const instead.",
            line: 1,
            column: 1,
            endLine: 1,
            endColumn: 4,
          },
        ],
      },
    ]);
    install("eslint", { stdout: eslintReport, exit: 1 });

    const { result } = review({ mode: "auto", enable: ["eslint"] });
    const noVar = result.findings.filter((f) => ruleIdFromEvidence(f.evidence)?.endsWith("no-var"));
    expect(
      noVar.map((f) => [ruleIdFromEvidence(f.evidence), f.locations[0]?.lines?.start]),
    ).toEqual([
      ["no-var", 2],
      ["eslint:no-var", 1],
    ]);
  });

  test("a diff review only reports the files the diff touches", () => {
    writeFileSync(join(repo, "app.py"), "import os\n");
    writeFileSync(join(repo, "other.py"), "import sys\n");
    install("ruff", {
      stdout: JSON.stringify(
        ["app.py", "other.py"].map((file) => ({
          code: "F401",
          message: "unused import",
          filename: join(repo, file),
          location: { row: 1, column: 8 },
        })),
      ),
    });
    const diff = ["--- a/app.py", "+++ b/app.py", "@@ -0,0 +1 @@", "+import os", ""].join("\n");

    const { result } = review({ mode: "auto" }, { scope: { kind: "diff", base: undefined, diff } });
    const files = result.findings
      .filter((f) => ruleIdFromEvidence(f.evidence) === "ruff:F401")
      .map((f) => f.locations[0]?.file);
    expect(files).toEqual(["app.py"]);
  });
});

describe("the ledger and analyzers that come and go", () => {
  const entries = () =>
    (
      JSON.parse(readFileSync(join(repo, ".debuggatha", "ledger.json"), "utf8")) as {
        entries: { status: string; fingerprint: { ruleId?: string } }[];
      }
    ).entries.filter((entry) => entry.fingerprint.ruleId?.startsWith("ruff:"));

  test("a review without the tool does not call its findings fixed; a review with it does", () => {
    writeFileSync(join(repo, "app.py"), "import os\n");
    install("ruff", { stdout: ruffReport(["F401", 1]) });
    review({ mode: "auto" });
    expect(entries().map((entry) => entry.status)).toEqual(["open"]);

    // Same repository, analyzers off: the tool said nothing, so nothing was fixed.
    const without = review(undefined);
    expect(without.changes.fixed).toEqual([]);
    expect(entries().map((entry) => entry.status)).toEqual(["open"]);

    // The tool runs and the finding is gone: now it is fixed.
    install("ruff", { stdout: "[]" });
    const fixed = review({ mode: "auto" });
    expect(fixed.changes.fixed.map((ref) => ref.ruleId)).toEqual(["ruff:F401"]);
    expect(entries().map((entry) => entry.status)).toEqual(["resolved"]);
  });

  test("a tool that failed does not resolve its findings either", () => {
    writeFileSync(join(repo, "app.py"), "import os\n");
    install("ruff", { stdout: ruffReport(["F401", 1]) });
    review({ mode: "auto" });

    install("ruff", { exit: 2 });
    review({ mode: "auto" });
    expect(entries().map((entry) => entry.status)).toEqual(["open"]);
  });
});

describe("Biome, run for real", () => {
  const biomeBin = join(import.meta.dir, "../../../../node_modules/.bin/biome");
  const installed = existsSync(biomeBin);

  test.skipIf(!installed)("lints a project with the repository's own installed Biome", () => {
    mkdirSync(join(repo, "node_modules", ".bin"), { recursive: true });
    symlinkSync(biomeBin, join(repo, "node_modules", ".bin", "biome"));
    writeFileSync(
      join(repo, "biome.json"),
      JSON.stringify({ linter: { rules: { recommended: true } } }),
    );
    writeFileSync(join(repo, "t.ts"), "export function f(a: any) {\n  return a == null;\n}\n");
    // Installed through npm, the `biome` command is a Node script, so Node has to be on the PATH.
    const nodeDir = savedPath?.split(delimiter).find((dir) => existsSync(join(dir, "node")));
    if (nodeDir) process.env.PATH = [bin, nodeDir, "/usr/bin", "/bin"].join(delimiter);
    const { analyzers, result } = review({ mode: "auto" });

    const run = analyzers.find((item) => item.id === "biome");
    expect(run?.status).toBe("ran");
    expect(run?.version).toMatch(/^\d+\.\d+/);
    const rules = result.findings.map((f) => ruleIdFromEvidence(f.evidence));
    expect(rules).toContain("biome:lint/suspicious/noExplicitAny");
    // Biome's `noExplicitAny` on the same line replaced the built-in `no-explicit-any`.
    expect(rules).not.toContain("no-explicit-any");
  });
});
