import { afterAll, describe, expect, it } from "bun:test";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const BIN = resolve(import.meta.dir, "bin.ts");
const ESC = String.fromCharCode(27);
const cleanup: string[] = [];

const BAD = 'var x: any = 1;\nif (x == 2) { eval("1"); }\nconst password = "hunter22222";\n';

function makeRepo(sourceLines = BAD): string {
  const dir = mkdtempSync(join(tmpdir(), "debuggatha-cli-e2e-"));
  cleanup.push(dir);
  mkdirSync(join(dir, "src"));
  writeFileSync(
    join(dir, "package.json"),
    JSON.stringify({ name: "fixture", devDependencies: { typescript: "^5.0.0" } }),
  );
  writeFileSync(join(dir, "src", "bad.ts"), sourceLines);
  return dir;
}

function run(
  args: string[],
  cwd: string,
  options: { env?: Record<string, string>; stdin?: string } = {},
) {
  const result = Bun.spawnSync([process.execPath, BIN, ...args], {
    cwd,
    env: { PATH: process.env.PATH ?? "", HOME: process.env.HOME ?? "", ...options.env },
    stdin: options.stdin === undefined ? "ignore" : new TextEncoder().encode(options.stdin),
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

afterAll(() => {
  for (const dir of cleanup) rmSync(dir, { recursive: true, force: true });
});

describe("review report formats", () => {
  it("lists each finding with its rule and exits 2 by default", () => {
    const repo = makeRepo();
    const { code, out } = run(["review", "--files", "src/bad.ts"], repo);
    expect(code).toBe(2);
    expect(out).toContain("src/bad.ts:1");
    expect(out).toContain("no-explicit-any");
    expect(out).toContain("no-eval");
  });

  it("prints only JSON on stdout for --format json and the global --json", () => {
    const repo = makeRepo();
    for (const args of [
      ["review", "--files", "src/bad.ts", "--format", "json"],
      ["--json", "review", "--files", "src/bad.ts"],
    ]) {
      const { out } = run(args, repo);
      const report = JSON.parse(out);
      expect(report.status).toBe("success");
      expect(report.reviewResult.findings.length).toBeGreaterThan(0);
      expect(report.persisted).toBe(true);
    }
  });

  it("emits SARIF 2.1.0 with one rule entry per rule and relative locations", () => {
    const repo = makeRepo();
    const { out } = run(["review", "--files", "src/bad.ts", "--format", "sarif"], repo);
    const sarif = JSON.parse(out);
    expect(sarif.version).toBe("2.1.0");
    const [runEntry] = sarif.runs;
    expect(runEntry.tool.driver.name).toBe("Debuggatha");
    expect(runEntry.results.length).toBeGreaterThan(0);
    const ruleIds = new Set(runEntry.tool.driver.rules.map((r: { id: string }) => r.id));
    expect(ruleIds.size).toBe(runEntry.tool.driver.rules.length);
    for (const result of runEntry.results) {
      expect(ruleIds.has(result.ruleId)).toBe(true);
      expect(["error", "warning", "note"]).toContain(result.level);
      expect(result.locations[0].physicalLocation.artifactLocation.uri).toBe("src/bad.ts");
    }
  });

  it("emits GitHub annotations", () => {
    const repo = makeRepo();
    const { out } = run(["review", "--files", "src/bad.ts", "--format", "github"], repo);
    const lines = out.trim().split("\n");
    expect(lines.length).toBeGreaterThan(0);
    for (const line of lines) {
      expect(line).toMatch(
        /^::(error|warning|notice) file=src\/bad\.ts,line=\d+,endLine=\d+,col=\d+,endColumn=\d+,title=/,
      );
    }
  });

  it("writes the report to --output and keeps stdout free of it", () => {
    const repo = makeRepo();
    const { out } = run(
      ["review", "--files", "src/bad.ts", "--format", "sarif", "--output", "report.sarif"],
      repo,
    );
    expect(out.trim()).toBe("");
    expect(existsSync(join(repo, "report.sarif"))).toBe(true);
  });

  it("uses no color under NO_COLOR, even when color is forced", () => {
    const repo = makeRepo();
    const colored = run(["review", "--files", "src/bad.ts"], repo, { env: { FORCE_COLOR: "1" } });
    expect(colored.out).toContain(`${ESC}[`);
    const plain = run(["review", "--files", "src/bad.ts"], repo, {
      env: { FORCE_COLOR: "1", NO_COLOR: "1" },
    });
    expect(plain.out).not.toContain(`${ESC}[`);
  });
});

describe("review exit policy and persistence", () => {
  it("fails only at or above --fail-on", () => {
    const anyOnly = makeRepo("var x: any = 1;\n");
    expect(run(["review", "--files", "src/bad.ts", "--fail-on", "none"], anyOnly).code).toBe(0);
    expect(run(["review", "--files", "src/bad.ts", "--fail-on", "critical"], anyOnly).code).toBe(0);
    expect(run(["review", "--files", "src/bad.ts", "--fail-on", "low"], anyOnly).code).toBe(2);
    const bad = makeRepo();
    // eval is high, not critical: only a leaked credential reaches critical.
    expect(run(["review", "--files", "src/bad.ts", "--fail-on", "high"], bad).code).toBe(2);
    expect(run(["review", "--files", "src/bad.ts", "--fail-on", "critical"], bad).code).toBe(0);
  });

  it("records findings in the ledger unless --no-persist", () => {
    const persisted = makeRepo();
    run(["review", "--files", "src/bad.ts"], persisted);
    expect(existsSync(join(persisted, ".debuggatha", "ledger.json"))).toBe(true);

    const readOnly = makeRepo();
    run(["review", "--files", "src/bad.ts", "--no-persist"], readOnly);
    expect(existsSync(join(readOnly, ".debuggatha"))).toBe(false);
  });

  it("rejects an invalid --fail-on value", () => {
    const { code, err } = run(["review", "--fail-on", "bogus"], makeRepo());
    expect(code).not.toBe(0);
    expect(err).toContain("critical");
  });

  it("documents its exit codes in --help", () => {
    const { out } = run(["review", "--help"], makeRepo());
    expect(out).toContain("Exit codes");
    expect(out).toContain("--fail-on");
  });
});

describe("review scope", () => {
  it("reviews a diff piped on stdin", () => {
    const repo = makeRepo("export const ok = 1;\n");
    const diff = [
      "diff --git a/src/bad.ts b/src/bad.ts",
      "--- a/src/bad.ts",
      "+++ b/src/bad.ts",
      "@@ -1 +1,2 @@",
      " export const ok = 1;",
      '+eval("1");',
      "",
    ].join("\n");
    const { code, out } = run(["review", "--diff", "-"], repo, { stdin: diff });
    expect(code).toBe(2);
    expect(out).toContain("no-eval");
    expect(out).toContain("src/bad.ts:2");
  });

  it("fails clearly when --diff - has nothing on stdin", () => {
    const { code, err } = run(["review", "--diff", "-"], makeRepo(), { stdin: "" });
    expect(code).toBe(1);
    expect(err).toContain("stdin");
  });

  it("reviews only what changed since --base in a real git repository", () => {
    const repo = makeRepo("var old: any = 1;\n");
    git(repo, "init", "-q", "-b", "main");
    git(repo, "add", "-A");
    git(repo, "commit", "-q", "-m", "base");
    writeFileSync(join(repo, "src", "bad.ts"), 'var old: any = 1;\neval("new");\n');

    const { code, out } = run(
      ["review", "--base", "HEAD", "--format", "json", "--no-persist"],
      repo,
    );
    expect(code).toBe(2);
    const rules = JSON.parse(out).reviewResult.findings.map(
      (f: { evidence: { ruleId?: string }[] }) => f.evidence.find((e) => e.ruleId)?.ruleId,
    );
    expect(rules).toContain("no-eval");
    expect(rules).not.toContain("no-explicit-any");
  });

  it("refuses a --base that would inject a shell command or a git option", () => {
    const repo = makeRepo();
    git(repo, "init", "-q", "-b", "main");
    for (const base of ["HEAD; touch pwned", "$(touch pwned)", "--output=pwned"]) {
      const { code, err } = run(["review", "--base", base], repo);
      expect(code).toBe(1);
      expect(err).toContain("Unsafe git ref");
    }
    expect(existsSync(join(repo, "pwned"))).toBe(false);
  });

  it("resolves --files against --cwd, not the process directory", () => {
    const repo = makeRepo();
    const elsewhere = mkdtempSync(join(tmpdir(), "debuggatha-cli-elsewhere-"));
    cleanup.push(elsewhere);
    const { code, out } = run(["-C", repo, "review", "--files", "src/bad.ts"], elsewhere);
    expect(code).toBe(2);
    expect(out).toContain("no-eval");
  });
});

describe("other commands", () => {
  it("prints a completion script for each shell", () => {
    const repo = makeRepo();
    const bash = run(["completion", "bash"], repo).out;
    expect(bash).toContain("complete -F _debuggatha debuggatha");
    expect(bash).toContain("--fail-on");
    expect(run(["completion", "zsh"], repo).out.startsWith("#compdef debuggatha")).toBe(true);
    expect(run(["completion", "fish"], repo).out).toContain("complete -c debuggatha");
    expect(run(["completion", "powershell"], repo).code).not.toBe(0);
  });

  it("applies packs that `policies` also reports", () => {
    const repo = makeRepo();
    const policy = JSON.parse(run(["--json", "policies"], repo).out);
    const review = JSON.parse(run(["--json", "review", "--files", "src/bad.ts"], repo).out);
    const reviewPacks = review.reviewResult.summary.appliedPacks.map((p: { id: string }) => p.id);
    for (const id of reviewPacks) {
      expect(policy.packRefs.map((p: { id: string }) => p.id)).toContain(id);
    }
  });
});

describe("trust: scope, guidance, suppression, adoption", () => {
  const rulesIn = (out: string): string[] =>
    JSON.parse(out).reviewResult.findings.map(
      (f: { evidence: { ruleId?: string }[] }) => f.evidence.find((e) => e.ruleId)?.ruleId,
    );

  it("reviews the whole repository with --all, even with uncommitted changes", () => {
    const repo = makeRepo("var old = 1;\n");
    git(repo, "init", "-q", "-b", "main");
    git(repo, "add", "-A");
    git(repo, "commit", "-q", "-m", "base");
    writeFileSync(join(repo, "src", "bad.ts"), 'var old = 1;\neval("new");\n');

    const diffOnly = rulesIn(run(["--json", "review", "--no-persist"], repo).out);
    expect(diffOnly).toEqual(["no-eval"]);

    const everything = rulesIn(run(["--json", "review", "--all", "--no-persist"], repo).out);
    expect(everything).toContain("no-eval");
    expect(everything).toContain("no-var");
  });

  it("refuses --all together with an explicit scope", () => {
    const { code, err } = run(["review", "--all", "--files", "src/bad.ts"], makeRepo());
    expect(code).not.toBe(0);
    expect(err).toContain("--all");
  });

  it("shows where the problem starts and how to fix it", () => {
    const repo = makeRepo("  var count = 0;\n");
    const { out } = run(["review", "--files", "src/bad.ts", "--no-persist"], repo);
    expect(out).toContain("src/bad.ts:1:3");
    expect(out).toContain("→ Use `const`, or `let` if the value is reassigned.");
  });

  it("reports columns in SARIF and JSON", () => {
    const repo = makeRepo("  var count = 0;\n");
    const sarif = JSON.parse(
      run(["review", "--files", "src/bad.ts", "--format", "sarif", "--no-persist"], repo).out,
    );
    expect(sarif.runs[0].results[0].locations[0].physicalLocation.region).toMatchObject({
      startLine: 1,
      startColumn: 3,
    });
    const json = JSON.parse(
      run(["--json", "review", "--files", "src/bad.ts", "--no-persist"], repo).out,
    );
    expect(json.reviewResult.findings[0].locations[0].columns).toEqual({ start: 3, end: 8 });
  });

  it("hides what a debuggatha-ignore comment accepts, and says so", () => {
    const repo = makeRepo("run(eval(x)); // debuggatha-ignore no-eval -- sandboxed\n");
    const { code, out } = run(["review", "--files", "src/bad.ts", "--no-persist"], repo);
    expect(code).toBe(0);
    expect(out).toContain("1 findings hidden by debuggatha-ignore comments");
    const json = JSON.parse(
      run(["--json", "review", "--files", "src/bad.ts", "--no-persist"], repo).out,
    );
    expect(json.suppressedInline).toEqual([
      { file: "src/bad.ts", line: 1, ruleId: "no-eval", reason: "sandboxed" },
    ]);
  });

  it("adoption mode: baseline the repository, then only new findings count", () => {
    const repo = makeRepo("var old = 1;\n");

    const created = run(["baseline", "create"], repo);
    expect(created.code).toBe(0);
    expect(created.out).toContain("Baseline recorded: 1 findings");
    expect(existsSync(join(repo, ".debuggatha", "baseline.json"))).toBe(true);
    expect(run(["baseline", "show"], repo).out).toContain("no-var");

    const quiet = run(["review", "--all", "--no-persist"], repo);
    expect(quiet.code).toBe(0);
    expect(quiet.out).toContain("1 findings accepted by the baseline are hidden");

    writeFileSync(join(repo, "src", "new.ts"), "var fresh = 1;\n");
    const later = run(["review", "--all", "--no-persist"], repo);
    expect(later.code).toBe(2);
    expect(later.out).toContain("src/new.ts");
    expect(later.out).not.toContain("src/bad.ts:1");

    const everything = JSON.parse(
      run(["--json", "review", "--all", "--no-persist", "--include-baselined"], repo).out,
    );
    expect(everything.reviewResult.findings).toHaveLength(2);

    expect(run(["baseline", "clear"], repo).out).toContain("Baseline removed");
    expect(run(["review", "--all", "--no-persist"], repo).code).toBe(2);
  });

  it("does not put the secret it found in the ledger it writes", () => {
    const secret = "sk_live_1234567890abcdef";
    const repo = makeRepo(`const key = "${secret}";\n`);
    const { code } = run(["review", "--files", "src/bad.ts"], repo);
    expect(code).toBe(2);
    expect(readFileSync(join(repo, ".debuggatha", "ledger.json"), "utf8")).not.toContain(secret);
  });
});

describe("external analyzers", () => {
  /** A `ruff` on its own PATH that reports one finding in `app.py`, as the real tool would. */
  function withRuff(repo: string) {
    const bin = mkdtempSync(join(tmpdir(), "debuggatha-cli-bin-"));
    cleanup.push(bin);
    const report = JSON.stringify([
      {
        code: "F401",
        message: "`os` imported but unused",
        filename: join(repo, "app.py"),
        location: { row: 1, column: 8 },
        end_location: { row: 1, column: 10 },
        url: "https://docs.astral.sh/ruff/rules/unused-import",
      },
    ]);
    const script = join(bin, "ruff");
    writeFileSync(
      script,
      `#!/bin/sh\nif [ "$1" = "--version" ]; then echo "ruff 0.15.1"; exit 0; fi\necho '${report}'\n`,
    );
    Bun.spawnSync(["chmod", "755", script]);
    return { PATH: `${bin}:${process.env.PATH ?? ""}` };
  }

  it("runs an installed analyzer and names the tool and its license beside the finding", () => {
    const repo = makeRepo("export const a = 1;\n");
    writeFileSync(join(repo, "app.py"), "import os\n");
    const { out, code } = run(["review", "--all", "--no-persist"], repo, { env: withRuff(repo) });

    expect(code).toBe(2);
    expect(out).toContain("app.py:1:8");
    expect(out).toContain("ruff:F401 · ruff 0.15.1, MIT");
    expect(out).toContain("Ruff 0.15.1 ran: 1 findings.");
  });

  it("puts the tool, version, and license in JSON and SARIF too", () => {
    const repo = makeRepo("export const a = 1;\n");
    writeFileSync(join(repo, "app.py"), "import os\n");
    const env = withRuff(repo);

    const json = JSON.parse(run(["--json", "review", "--all", "--no-persist"], repo, { env }).out);
    expect(json.analyzers.find((a: { id: string }) => a.id === "ruff")).toMatchObject({
      status: "ran",
      license: "MIT",
      version: "0.15.1",
    });

    const sarif = JSON.parse(
      run(["review", "--all", "--no-persist", "--format", "sarif"], repo, { env }).out,
    );
    expect(sarif.runs[0].results[0].properties.analyzer).toMatchObject({
      tool: "ruff",
      ruleId: "F401",
      license: "MIT",
    });
  });

  it("--no-analyzers runs only the built-in detectors", () => {
    const repo = makeRepo("export const a = 1;\n");
    writeFileSync(join(repo, "app.py"), "import os\n");
    const { out, code } = run(["review", "--all", "--no-persist", "--no-analyzers"], repo, {
      env: withRuff(repo),
    });
    expect(code).toBe(0);
    expect(out).not.toContain("ruff");
  });

  it("does not run a tool that executes project code unless it is named", () => {
    const repo = makeRepo("export const a = 1;\n");
    writeFileSync(join(repo, "eslint.config.js"), "export default [];\n");
    const bin = mkdtempSync(join(tmpdir(), "debuggatha-cli-bin-"));
    cleanup.push(bin);
    writeFileSync(
      join(bin, "eslint"),
      `#!/bin/sh\necho "eslint ran" > "${join(repo, "ran.txt")}"\necho '[]'\n`,
    );
    Bun.spawnSync(["chmod", "755", join(bin, "eslint")]);
    const env = { PATH: `${bin}:${process.env.PATH ?? ""}` };

    const skipped = run(["review", "--all", "--no-persist"], repo, { env });
    expect(skipped.out).toContain("ESLint skipped");
    expect(skipped.out).toContain("--analyzer eslint");
    expect(existsSync(join(repo, "ran.txt"))).toBe(false);

    run(["review", "--all", "--no-persist", "--analyzer", "eslint"], repo, { env });
    expect(existsSync(join(repo, "ran.txt"))).toBe(true);
  });

  it("rejects an analyzer it does not know", () => {
    const repo = makeRepo();
    const { code, err } = run(["review", "--analyzer", "nope"], repo);
    expect(code).not.toBe(0);
    expect(err).toContain('Unknown analyzer "nope"');
  });

  it("`analyzers` lists what is installed with its license", () => {
    const repo = makeRepo("export const a = 1;\n");
    writeFileSync(join(repo, "app.py"), "import os\n");
    const { out } = run(["analyzers"], repo, { env: withRuff(repo) });
    expect(out).toContain("ruff 0.15.1");
    expect(out).toContain("will run");
    expect(out).toContain("MIT");
    expect(out).toContain("does not include or install them");

    const json = JSON.parse(run(["--json", "analyzers"], repo, { env: withRuff(repo) }).out);
    expect(json.analyzers.map((a: { id: string }) => a.id)).toContain("gitleaks");
  });

  it("a baseline made with an analyzer covers what that analyzer finds", () => {
    const repo = makeRepo("export const a = 1;\n");
    writeFileSync(join(repo, "app.py"), "import os\n");
    const env = withRuff(repo);
    expect(run(["baseline", "create"], repo, { env }).code).toBe(0);
    expect(run(["review", "--all", "--no-persist"], repo, { env }).code).toBe(0);
  });
});

describe("semantic pass", () => {
  const OFF_BY_ONE =
    "export function total(items: number[]) {\n  let sum = 0;\n  for (let i = 0; i <= items.length; i++) sum += items[i];\n  return sum;\n}\n";

  /** A runtime that answers like Ollama, and remembers what it was sent. */
  function fakeOllama(answer: string) {
    const prompts: string[] = [];
    const server = Bun.serve({
      port: 0,
      hostname: "127.0.0.1",
      async fetch(request) {
        const path = new URL(request.url).pathname;
        if (path === "/api/tags") return Response.json({ models: [{ name: "fake:1b" }] });
        if (path === "/api/generate") {
          const body = (await request.json()) as { system?: string; prompt: string };
          prompts.push(`${body.system ?? ""}\n${body.prompt}`);
          // Ollama streams one JSON object per line.
          return new Response(
            `${JSON.stringify({ response: answer, done: false })}\n${JSON.stringify({ done: true })}\n`,
          );
        }
        return new Response("no", { status: 404 });
      },
    });
    return { server, prompts, url: `http://127.0.0.1:${server.port}` };
  }

  /** Async, because the fake runtime lives in this process and `spawnSync` would block it. */
  async function runAsync(args: string[], cwd: string) {
    const child = Bun.spawn([process.execPath, BIN, ...args], {
      cwd,
      env: { PATH: process.env.PATH ?? "", HOME: process.env.HOME ?? "" },
      stdout: "pipe",
      stderr: "pipe",
    });
    const [out, err] = await Promise.all([
      new Response(child.stdout).text(),
      new Response(child.stderr).text(),
    ]);
    return { code: await child.exited, out, err };
  }

  const claim = JSON.stringify({
    issues: [
      {
        line: 3,
        quote: "i <= items.length",
        title: "Off by one",
        why: "Reads past the end.",
        category: "reliability",
        severity: "medium",
      },
    ],
  });

  it("shows what the model suspects, labeled as a suspicion with the model that raised it", async () => {
    const repo = makeRepo(OFF_BY_ONE);
    const { server, url } = fakeOllama(claim);
    try {
      const { out, code } = await runAsync(
        [
          "review",
          "--files",
          "src/bad.ts",
          "--no-analyzers",
          "--semantic",
          "ollama",
          "--semantic-url",
          url,
        ],
        repo,
      );
      expect(code).toBe(2);
      expect(out).toContain("src/bad.ts:3");
      expect(out).toContain("semantic:off-by-one · model suspicion, ollama fake:1b");
      expect(out).toContain("Semantic pass (ollama fake:1b): 1 suspected issue raised, 0 dropped");
    } finally {
      server.stop(true);
    }
  });

  it("puts the semantic run in JSON and the model's part in SARIF", async () => {
    const repo = makeRepo(OFF_BY_ONE);
    const { server, url } = fakeOllama(claim);
    try {
      const json = JSON.parse(
        (
          await runAsync(
            [
              "--json",
              "review",
              "--files",
              "src/bad.ts",
              "--no-analyzers",
              "--semantic",
              "ollama",
              "--semantic-url",
              url,
            ],
            repo,
          )
        ).out,
      );
      expect(json.semantic).toMatchObject({ provider: "ollama", model: "fake:1b", detected: 1 });

      const sarif = JSON.parse(
        (
          await runAsync(
            [
              "review",
              "--files",
              "src/bad.ts",
              "--no-analyzers",
              "--format",
              "sarif",
              "--semantic",
              "ollama",
              "--semantic-url",
              url,
            ],
            repo,
          )
        ).out,
      );
      const result = sarif.runs[0].results.find((r: { ruleId: string }) =>
        r.ruleId.startsWith("semantic:"),
      );
      expect(result.properties.semantic).toMatchObject({ role: "detection", provider: "ollama" });
      expect(result.properties.confidence).toBe("low");
    } finally {
      server.stop(true);
    }
  });

  it("does not send a line that may hold a secret to the model", async () => {
    const secret = "sk_live_1234567890abcdef";
    const repo = makeRepo(`const key = "${secret}";\nexport const ok = 1;\n`);
    const { server, prompts, url } = fakeOllama('{"issues":[]}');
    try {
      await runAsync(
        [
          "review",
          "--files",
          "src/bad.ts",
          "--no-analyzers",
          "--semantic",
          "ollama",
          "--semantic-url",
          url,
        ],
        repo,
      );
      expect(prompts.length).toBeGreaterThan(0);
      expect(prompts.join("\n")).not.toContain(secret);
    } finally {
      server.stop(true);
    }
  });

  it("refuses a runtime that is not on this machine, and says so when none is running", async () => {
    const repo = makeRepo(OFF_BY_ONE);
    const remote = await runAsync(
      [
        "review",
        "--files",
        "src/bad.ts",
        "--semantic",
        "ollama",
        "--semantic-url",
        "https://example.com",
      ],
      repo,
    );
    expect(remote.code).toBe(1);
    expect(remote.err + remote.out).toContain("only to a model on this machine");

    const { server, url } = fakeOllama("{}");
    server.stop(true);
    const down = await runAsync(
      ["review", "--files", "src/bad.ts", "--semantic", "ollama", "--semantic-url", url],
      repo,
    );
    expect(down.code).toBe(1);
    expect(down.err + down.out).toContain("not answering");
  });

  it("without --semantic, no model is contacted and nothing is labeled", () => {
    const repo = makeRepo(OFF_BY_ONE);
    const { out } = run(["review", "--files", "src/bad.ts", "--no-analyzers"], repo);
    expect(out).not.toContain("model suspicion");
    expect(out).not.toContain("Semantic pass");
  });
});
