import { afterAll, describe, expect, it } from "bun:test";
import {
  chmodSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { githubWorkflow } from "./commands/init.js";

/*
 * The everyday workflow around a review: a config the team shares, reviewing only what changed,
 * failing only on what a change introduced, a report for a pull request, and `init`.
 */

const BIN = resolve(import.meta.dir, "bin.ts");
const cleanup: string[] = [];
afterAll(() => {
  for (const dir of cleanup) rmSync(dir, { recursive: true, force: true });
});

function run(args: string[], cwd: string, env: Record<string, string> = {}) {
  const result = Bun.spawnSync([process.execPath, BIN, ...args], {
    cwd,
    env: { PATH: process.env.PATH ?? "", HOME: process.env.HOME ?? "", ...env },
    stdin: "ignore",
  });
  return { code: result.exitCode, out: result.stdout.toString(), err: result.stderr.toString() };
}

function git(cwd: string, ...args: string[]) {
  const result = Bun.spawnSync(
    ["git", "-c", "user.name=t", "-c", "user.email=t@t", "-c", "commit.gpgsign=false", ...args],
    { cwd },
  );
  if (result.exitCode !== 0) throw new Error(result.stderr.toString());
}

function write(root: string, files: Record<string, string>) {
  for (const [path, content] of Object.entries(files)) {
    mkdirSync(dirname(join(root, path)), { recursive: true });
    writeFileSync(join(root, path), content);
  }
}

/** A committed TypeScript repository with one old problem already in it. */
function repoWithHistory(): string {
  const dir = mkdtempSync(join(tmpdir(), "debuggatha-workflow-"));
  cleanup.push(dir);
  write(dir, {
    "package.json": JSON.stringify({ name: "app", devDependencies: { typescript: "^5.0.0" } }),
    "src/old.ts": "var legacy = 1;\nexport const ok = legacy;\n",
    "src/clean.ts": "export const clean = 1;\n",
  });
  git(dir, "init", "-q", "-b", "main");
  git(dir, "add", "-A");
  git(dir, "commit", "-q", "-m", "base");
  return dir;
}

const NO_TOOLS = ["--no-analyzers", "--no-persist"];

describe("the repository's config", () => {
  it("sets defaults that a flag still overrides", () => {
    const repo = repoWithHistory();
    write(repo, { ".debuggatha/config.json": JSON.stringify({ review: { failOn: "critical" } }) });

    // `var` is low severity: below the configured bar, so the run passes...
    expect(run(["review", "--all", ...NO_TOOLS], repo).code).toBe(0);
    // ...until the person asks for a stricter one.
    expect(run(["review", "--all", "--fail-on", "low", ...NO_TOOLS], repo).code).toBe(2);
  });

  it("picks the report format and extra packs the team chose", () => {
    const repo = repoWithHistory();
    write(repo, {
      ".debuggatha/config.json": JSON.stringify({
        review: { format: "markdown", failOn: "none" },
      }),
    });
    const { out } = run(["review", "--all", ...NO_TOOLS], repo);
    expect(out).toContain("<!-- debuggatha-report -->");
  });

  it("is found from a package inside the repository, and stops at the repository's root", () => {
    const repo = repoWithHistory();
    write(repo, {
      ".debuggatha/config.json": JSON.stringify({ review: { failOn: "none" } }),
      "packages/web/package.json": JSON.stringify({ name: "web" }),
      "packages/web/a.ts": "var x = 1;\n",
    });
    const inside = run(["-C", join(repo, "packages/web"), "review", "--all", ...NO_TOOLS], repo);
    expect(inside.code).toBe(0);
  });

  it("can narrow analyzers but never enable one, and never turn on a model", () => {
    const repo = repoWithHistory();
    write(repo, {
      ".debuggatha/config.json": JSON.stringify({
        analyzers: { mode: "auto", enable: ["eslint"] },
      }),
    });
    const enabled = run(["review", "--all", "--no-persist"], repo);
    expect(enabled.code).toBe(1);
    expect(enabled.err + enabled.out).toContain('"analyzers.enable" is not allowed in the file');
    expect(enabled.err + enabled.out).toContain("--analyzer");

    write(repo, {
      ".debuggatha/config.json": JSON.stringify({ semantic: { provider: "ollama" } }),
    });
    const semantic = run(["review", "--all", "--no-persist"], repo);
    expect(semantic.code).toBe(1);
    expect(semantic.err + semantic.out).toContain('"semantic" is not allowed in the file');
  });

  it("names a setting it does not know, and a value that is wrong", () => {
    const repo = repoWithHistory();
    write(repo, { ".debuggatha/config.json": JSON.stringify({ review: { failon: "high" } }) });
    const typo = run(["review", "--all", "--no-persist"], repo);
    expect(typo.code).toBe(1);
    expect(typo.err + typo.out).toContain('"review.failon" is not a setting');

    write(repo, { ".debuggatha/config.json": JSON.stringify({ review: { failOn: "severe" } }) });
    const value = run(["review", "--all", "--no-persist"], repo);
    expect(value.code).toBe(1);
    expect(value.err + value.out).toContain("review.failOn");

    write(repo, { ".debuggatha/config.json": "{ not json" });
    expect(run(["review", "--all", "--no-persist"], repo).err).toContain("is not valid JSON");
  });

  it("tolerates what `init` used to write, and says it was never read", () => {
    const repo = repoWithHistory();
    write(repo, {
      ".debuggatha/config.json": JSON.stringify({
        "//": "Debuggatha Configuration",
        review: { defaultPolicy: "default", defaultDepth: "full" },
      }),
    });
    const { out, err, code } = run(["review", "--all", "--fail-on", "none", ...NO_TOOLS], repo);
    expect(code).toBe(0);
    // Warnings go to stderr.
    expect(out + err).toContain("were never read and are ignored");
  });

  it("can switch analyzers off or skip one, and `analyzers` shows it", () => {
    const repo = repoWithHistory();
    write(repo, {
      "app.py": "import os\n",
      ".debuggatha/config.json": JSON.stringify({ analyzers: { disable: ["ruff"] } }),
    });
    const entry = (
      JSON.parse(run(["--json", "analyzers"], repo).out) as {
        analyzers: { id: string; willRun: boolean; reason?: string }[];
      }
    ).analyzers.find((a) => a.id === "ruff");
    expect(entry?.willRun).toBe(false);
    expect(entry?.reason).toContain("disabled in the repository's configuration");

    write(repo, { ".debuggatha/config.json": JSON.stringify({ analyzers: { mode: "off" } }) });
    const off = JSON.parse(run(["--json", "review", "--all", "--no-persist"], repo).out);
    expect(off.analyzers).toEqual([]);
  });
});

describe("reviewing only what changed", () => {
  it("--changed-since reviews the whole files that changed, new ones included", () => {
    const repo = repoWithHistory();
    write(repo, {
      "src/old.ts": "var legacy = 1;\nvar added = 2;\nexport const ok = legacy + added;\n",
      "src/brand-new.ts": "var fresh = 1;\n",
    });
    const { out } = run(
      ["review", "--changed-since", "HEAD", "--fail-on", "none", ...NO_TOOLS],
      repo,
    );
    // Whole file: the old problem in a changed file is part of the review.
    expect(out).toContain("src/old.ts:1");
    expect(out).toContain("src/old.ts:2");
    expect(out).toContain("src/brand-new.ts:1");
    expect(out).not.toContain("src/clean.ts");
    expect(out).toContain("Reviewing 2 files changed since HEAD");
  });

  it("says so, and passes, when nothing changed", () => {
    const repo = repoWithHistory();
    const text = run(["review", "--changed-since", "HEAD", ...NO_TOOLS], repo);
    expect(text.code).toBe(0);
    expect(text.out).toContain("No changes since HEAD.");
    const json = JSON.parse(
      run(["--json", "review", "--changed-since", "HEAD", ...NO_TOOLS], repo).out,
    );
    expect(json).toMatchObject({ status: "success", reviewResult: { findings: [] } });
    const sarif = JSON.parse(
      run(["review", "--changed-since", "HEAD", "--format", "sarif", ...NO_TOOLS], repo).out,
    );
    expect(sarif.runs[0].results).toEqual([]);
  });

  it("refuses a ref that would inject a shell command or a git option", () => {
    const repo = repoWithHistory();
    const { code, err, out } = run(
      ["review", "--changed-since", "HEAD; touch pwned", ...NO_TOOLS],
      repo,
    );
    expect(code).toBe(1);
    expect(err + out).toContain("Unsafe git ref");
    expect(existsSync(join(repo, "pwned"))).toBe(false);
  });

  it("can not be combined with an explicit scope", () => {
    const repo = repoWithHistory();
    const { code, err } = run(["review", "--changed-since", "HEAD", "--all"], repo);
    expect(code).not.toBe(0);
    expect(err).toContain("cannot be used with");
  });
});

describe("failing only on what a change introduced", () => {
  it("passes when the only findings were already there", () => {
    const repo = repoWithHistory();
    write(repo, { "src/clean.ts": "export const clean = 2;\n" });
    const { code, out } = run(
      ["review", "--changed-since", "HEAD", "--fail-on-new", "--fail-on", "low", ...NO_TOOLS],
      repo,
    );
    expect(code).toBe(0);
    expect(out).toContain("0 findings"); // src/old.ts was not touched, so it was not reviewed
    expect(out).not.toContain("src/old.ts");
  });

  it("fails on a finding on a line the change added, and reports the old one without counting it", () => {
    const repo = repoWithHistory();
    write(repo, {
      "src/old.ts": "var legacy = 1;\nvar added = 2;\nexport const ok = legacy + added;\n",
    });
    const { code, out } = run(
      ["review", "--changed-since", "HEAD", "--fail-on-new", "--fail-on", "low", ...NO_TOOLS],
      repo,
    );
    expect(code).toBe(2);
    expect(out).toContain("src/old.ts:1"); // reported
    expect(out).toContain("src/old.ts:2"); // counted
    expect(out).toContain("1 more finding is outside the lines this change touched");
  });

  it("does not fail when the change only touches lines that are fine, even in a file with old findings", () => {
    const repo = repoWithHistory();
    write(repo, {
      "src/old.ts": "var legacy = 1;\nexport const ok = legacy;\nexport const more = 1;\n",
    });
    const { code } = run(
      ["review", "--changed-since", "HEAD", "--fail-on-new", "--fail-on", "low", ...NO_TOOLS],
      repo,
    );
    expect(code).toBe(0);
  });

  it("counts every line of a file git does not track yet", () => {
    const repo = repoWithHistory();
    write(repo, { "src/untracked.ts": "var a = 1;\n" });
    const { code } = run(
      ["review", "--changed-since", "HEAD", "--fail-on-new", "--fail-on", "low", ...NO_TOOLS],
      repo,
    );
    expect(code).toBe(2);
  });

  it("works with --base too, measuring against that ref", () => {
    const repo = repoWithHistory();
    git(repo, "checkout", "-q", "-b", "feature");
    write(repo, {
      "src/old.ts": "var legacy = 1;\nvar added = 2;\nexport const ok = legacy + added;\n",
    });
    git(repo, "add", "-A");
    git(repo, "commit", "-q", "-m", "change");
    const { code } = run(
      ["review", "--base", "main", "--fail-on-new", "--fail-on", "low", ...NO_TOOLS],
      repo,
    );
    expect(code).toBe(2);
  });

  it("puts how many findings count in the JSON", () => {
    const repo = repoWithHistory();
    write(repo, { "src/clean.ts": "export const clean = 2;\n" });
    const json = JSON.parse(
      run(["--json", "review", "--changed-since", "HEAD", "--fail-on-new", ...NO_TOOLS], repo).out,
    );
    expect(json.countedFindings).toBe(0);
  });
});

describe("a report for a pull request", () => {
  it("leads with the counts, then a table of where and what, marked so a job can update it", () => {
    const repo = repoWithHistory();
    write(repo, {
      "src/bad.ts": 'var a | b = 1;\nvar x: any = 1;\nif (x == 2) { eval("1"); }\n',
    });
    const { out } = run(
      ["review", "--files", "src/bad.ts", "--format", "markdown", "--fail-on", "none", ...NO_TOOLS],
      repo,
    );
    expect(out.startsWith("<!-- debuggatha-report -->")).toBe(true);
    expect(out).toContain("## Debuggatha review");
    expect(out).toMatch(/\*\*\d+ findings\*\*: /);
    expect(out).toContain("| Severity | Where | Finding | Rule |");
    expect(out).toContain("`src/bad.ts:3`");
    expect(out).toContain("no-eval");
    expect(out).toContain("<details><summary>How to fix</summary>");
  });

  it("links each finding to GitHub when it runs in Actions", () => {
    const repo = repoWithHistory();
    const { out } = run(
      ["review", "--all", "--format", "markdown", "--fail-on", "none", ...NO_TOOLS],
      repo,
      {
        GITHUB_SERVER_URL: "https://github.com",
        GITHUB_REPOSITORY: "acme/app",
        GITHUB_SHA: "abc123",
      },
    );
    expect(out).toContain("(https://github.com/acme/app/blob/abc123/src/old.ts#L1)");
  });

  it("says how many findings the change did not count", () => {
    const repo = repoWithHistory();
    // A clean line added to a file that already has an old finding.
    write(repo, {
      "src/old.ts": "var legacy = 1;\nexport const ok = legacy;\nexport const more = 2;\n",
    });
    const { out } = run(
      [
        "review",
        "--changed-since",
        "HEAD",
        "--fail-on-new",
        "--format",
        "markdown",
        "--fail-on",
        "low",
        ...NO_TOOLS,
      ],
      repo,
    );
    expect(out).toContain("<!-- debuggatha-report -->");
    expect(out).toContain("outside the lines this change touched");
  });

  it("escapes what would break a table", () => {
    const repo = repoWithHistory();
    write(repo, { "src/bad.ts": "var a = 1;\n" });
    const { out } = run(
      ["review", "--files", "src/bad.ts", "--format", "markdown", "--fail-on", "none", ...NO_TOOLS],
      repo,
    );
    for (const row of out
      .split("\n")
      .filter((line) => line.startsWith("| low") || line.startsWith("| medium"))) {
      expect(row.replace(/\\\|/g, "").split("|").length).toBe(6);
    }
  });
});

describe("init", () => {
  it("leaves a new repository with a config and a CI workflow, and touches nothing else", () => {
    const repo = repoWithHistory();
    const { code, out } = run(["init"], repo);
    expect(code).toBe(0);
    expect(out).toContain("Created .debuggatha/config.json");
    expect(out).toContain("Created .github/workflows/debuggatha.yml");

    expect(JSON.parse(readFileSync(join(repo, ".debuggatha/config.json"), "utf8"))).toEqual({
      review: { failOn: "high" },
      analyzers: { mode: "auto" },
    });
    const workflow = readFileSync(join(repo, ".github/workflows/debuggatha.yml"), "utf8");
    expect(workflow).toContain("pull_request");
    expect(workflow).toContain("fetch-depth: 0");
    expect(workflow).toContain("--changed-since");
    expect(workflow).toContain("--fail-on-new");
    expect(workflow).toContain("--fail-on high");
    expect(workflow).toMatch(/@sxnnyside\/debuggatha-cli@\d+\.\d+\.\d+/);
    expect(existsSync(join(repo, ".git/hooks/pre-commit"))).toBe(false);
  });

  it("the config it writes is one the CLI accepts", () => {
    const repo = repoWithHistory();
    run(["init"], repo);
    // High is above `var`, so a review with the fresh config passes.
    expect(run(["review", "--all", ...NO_TOOLS], repo).code).toBe(0);
  });

  it("the command in the workflow is a real command that works on a pull request", () => {
    const repo = repoWithHistory();
    git(repo, "checkout", "-q", "-b", "feature");
    write(repo, { "src/eval.ts": "export const run = (s: string) => eval(s);\n" });
    git(repo, "add", "-A");
    git(repo, "commit", "-q", "-m", "risky");

    // Take the command from the workflow itself, with `main` where GitHub puts the base branch.
    const workflow = githubWorkflow("high", "0.0.0");
    const command =
      /npx --yes @sxnnyside\/debuggatha-cli@\S+ (review[\s\S]*?debuggatha-report\.md)/.exec(
        workflow,
      )?.[1] ?? "";
    expect(command).not.toBe("");
    const args = command
      .replaceAll("\\\n", " ")
      .replace('"origin/${{ github.base_ref }}"', "main")
      .split(/\s+/)
      .filter(Boolean);
    const reportPath = join(repo, "debuggatha-report.md");
    args[args.indexOf("debuggatha-report.md")] = reportPath;

    const { code } = run(args, repo);
    expect(code).toBe(2); // eval is high: the change introduced it
    const report = readFileSync(reportPath, "utf8");
    expect(report).toContain("<!-- debuggatha-report -->");
    expect(report).toContain("no-eval");
  });

  it("does not overwrite what is there, unless asked", () => {
    const repo = repoWithHistory();
    write(repo, {
      ".debuggatha/config.json": JSON.stringify({ review: { failOn: "medium" } }),
      ".github/workflows/debuggatha.yml": "# mine\n",
    });
    const { out } = run(["init"], repo);
    expect(out).toContain("Kept .debuggatha/config.json");
    expect(out).toContain("Kept .github/workflows/debuggatha.yml");
    expect(readFileSync(join(repo, ".github/workflows/debuggatha.yml"), "utf8")).toBe("# mine\n");

    const forced = run(["init", "--force", "--fail-on", "critical"], repo);
    expect(forced.out).toContain("Updated .github/workflows/debuggatha.yml");
    expect(readFileSync(join(repo, ".github/workflows/debuggatha.yml"), "utf8")).toContain(
      "--fail-on critical",
    );
  });

  it("--ci none writes no workflow, and --fail-on is carried through", () => {
    const repo = repoWithHistory();
    run(["init", "--ci", "none", "--fail-on", "medium"], repo);
    expect(existsSync(join(repo, ".github"))).toBe(false);
    expect(
      JSON.parse(readFileSync(join(repo, ".debuggatha/config.json"), "utf8")).review.failOn,
    ).toBe("medium");
  });

  it("--hooks installs an executable pre-commit hook, and never replaces someone else's", () => {
    const repo = repoWithHistory();
    run(["init", "--hooks", "--ci", "none"], repo);
    const hook = join(repo, ".git/hooks/pre-commit");
    expect(readFileSync(hook, "utf8")).toContain("# debuggatha-hook");
    expect(readFileSync(hook, "utf8")).toContain("--fail-on-new");
    expect(statSync(hook).mode & 0o111).not.toBe(0);

    // Running init again updates its own hook...
    expect(run(["init", "--hooks", "--ci", "none"], repo).out).toContain("Updated");

    // ...but a hook written by someone else stays.
    writeFileSync(hook, "#!/bin/sh\necho theirs\n");
    chmodSync(hook, 0o755);
    const kept = run(["init", "--hooks", "--ci", "none"], repo);
    expect(kept.out).toContain("did not write is already there");
    expect(readFileSync(hook, "utf8")).toContain("theirs");
  });

  it("the hook it installs runs the local CLI, then the global one, and never downloads", () => {
    const repo = repoWithHistory();
    run(["init", "--hooks", "--ci", "none", "--fail-on", "medium"], repo);
    const hook = join(repo, ".git/hooks/pre-commit");
    const hookEnv = { PATH: "/usr/bin:/bin" };
    const sh = (env: Record<string, string>) =>
      Bun.spawnSync(["sh", hook], { cwd: repo, env, stdout: "pipe", stderr: "pipe" });

    // Nothing installed: the commit goes through, with a note, and nothing is fetched.
    const bare = sh(hookEnv);
    expect(bare.exitCode).toBe(0);
    expect(bare.stderr.toString()).toContain("not installed here");

    // A CLI in the project wins, and is given the flags the hook was written with.
    const marker = join(repo, "ran.txt");
    const fake = (path: string, code: number) => {
      mkdirSync(dirname(path), { recursive: true });
      writeFileSync(path, `#!/bin/sh\necho "$@" > "${marker}"\nexit ${code}\n`);
      chmodSync(path, 0o755);
    };
    fake(join(repo, "node_modules/.bin/debuggatha"), 2);
    const local = sh(hookEnv);
    expect(local.exitCode).toBe(2); // the hook's exit is the CLI's: a finding blocks the commit
    expect(readFileSync(marker, "utf8").trim()).toBe(
      "review --fail-on-new --fail-on medium --no-persist --quiet",
    );

    // Without a local one, a global CLI on the PATH is used.
    rmSync(join(repo, "node_modules"), { recursive: true });
    rmSync(marker);
    const bin = mkdtempSync(join(tmpdir(), "debuggatha-hook-bin-"));
    cleanup.push(bin);
    fake(join(bin, "debuggatha"), 0);
    expect(sh({ PATH: `${bin}:/usr/bin:/bin` }).exitCode).toBe(0);
    expect(existsSync(marker)).toBe(true);
  });

  it("leaves the hooks to a hook manager that owns them", () => {
    const repo = repoWithHistory();
    git(repo, "config", "core.hooksPath", ".husky");
    const { out } = run(["init", "--hooks", "--ci", "none"], repo);
    expect(out).toContain('hooks from ".husky"');
    expect(existsSync(join(repo, ".git/hooks/pre-commit"))).toBe(false);
  });

  it("recognizes a monorepo, and reports every step as JSON", () => {
    const repo = repoWithHistory();
    write(repo, { "pnpm-workspace.yaml": "packages:\n  - 'packages/*'\n" });
    const json = JSON.parse(run(["--json", "init"], repo).out);
    expect(json.monorepo).toContain("pnpm workspace");
    expect(json.steps.map((step: { file: string }) => step.file)).toContain(
      ".debuggatha/config.json",
    );
  });
});

describe("trends", () => {
  it("tells a repository with no history how to start one", () => {
    const repo = repoWithHistory();
    expect(run(["trends"], repo).out).toContain("The ledger is empty");
  });

  it("counts what was introduced and resolved, and where what is open lives", () => {
    const repo = repoWithHistory();
    write(repo, { "src/two.ts": "var a = 1;\nvar b: any = 2;\n" });
    run(["review", "--all", "--no-analyzers"], repo);
    // Fix one file: its findings are resolved by the next review.
    write(repo, { "src/old.ts": "export const ok = 1;\n" });
    run(["review", "--all", "--no-analyzers"], repo);

    const { trends } = JSON.parse(run(["--json", "trends", "--since", "4w"], repo).out);
    expect(trends.days).toBe(28);
    expect(trends.introduced).toBeGreaterThanOrEqual(3);
    expect(trends.resolved).toBe(1);
    expect(trends.open.total).toBe(trends.introduced - trends.resolved);
    expect(trends.topFiles[0].file).toBe("src/two.ts");
    expect(trends.meanDaysToResolve).toBeGreaterThanOrEqual(0);
    expect(trends.weeks).toHaveLength(4);
    expect(
      trends.weeks.reduce((sum: number, week: { introduced: number }) => sum + week.introduced, 0),
    ).toBe(trends.introduced);

    const text = run(["trends"], repo).out;
    expect(text).toContain("open now");
    expect(text).toContain("introduced");
    expect(text).toContain("src/two.ts");
  });

  it("rejects a window it cannot read", () => {
    const repo = repoWithHistory();
    const { code, err } = run(["trends", "--since", "soon"], repo);
    expect(code).not.toBe(0);
    expect(err).toContain('Expected a window like "30d" or "8w"');
  });
});
