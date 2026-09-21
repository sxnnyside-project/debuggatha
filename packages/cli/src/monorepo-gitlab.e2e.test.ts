import { afterAll, describe, expect, it } from "bun:test";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { githubWorkflow, gitlabPipeline } from "./commands/init.js";
import { globToRegExp, matchesAny } from "./path-policy.js";

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

/** A monorepo: an API, a web app, a legacy package, and generated code, each with a `var`. */
function monorepo(): string {
  const dir = mkdtempSync(join(tmpdir(), "debuggatha-monorepo-"));
  cleanup.push(dir);
  const manifest = (name: string) =>
    JSON.stringify({ name, devDependencies: { typescript: "^5.0.0" } });
  write(dir, {
    "package.json": JSON.stringify({ name: "root", workspaces: ["packages/*"] }),
    "packages/api/package.json": manifest("api"),
    "packages/api/src/a.ts": "var api = 1;\n",
    "packages/web/package.json": manifest("web"),
    "packages/web/src/w.ts": "var web = 1;\n",
    "packages/legacy/package.json": manifest("legacy"),
    "packages/legacy/src/l.ts": "var legacy = 1;\n",
    "packages/generated/package.json": manifest("generated"),
    "packages/generated/src/g.ts": "var generated = 1;\n",
  });
  git(dir, "init", "-q", "-b", "main");
  git(dir, "add", "-A");
  git(dir, "commit", "-q", "-m", "base");
  return dir;
}

const NO_TOOLS = ["--no-analyzers", "--no-persist"];
const config = (repo: string, value: unknown) =>
  write(repo, { ".debuggatha/config.json": JSON.stringify(value) });

describe("path patterns", () => {
  const matches = (glob: string, file: string) => globToRegExp(glob).test(file);

  it("`**` crosses directories, `*` stays in one, `?` is one character", () => {
    expect(matches("packages/**/*.ts", "packages/api/src/a.ts")).toBe(true);
    expect(matches("packages/**/*.ts", "packages/a.ts")).toBe(true);
    expect(matches("packages/*/a.ts", "packages/api/a.ts")).toBe(true);
    expect(matches("packages/*/a.ts", "packages/api/src/a.ts")).toBe(false);
    expect(matches("src/?.ts", "src/a.ts")).toBe(true);
    expect(matches("src/?.ts", "src/ab.ts")).toBe(false);
  });

  it("a directory covers what is inside it, and a bare name matches at any depth", () => {
    expect(matches("packages/legacy", "packages/legacy/src/l.ts")).toBe(true);
    expect(matches("packages/legacy", "packages/legacy-two/src/l.ts")).toBe(false);
    expect(matches("*.generated.ts", "packages/x/src/a.generated.ts")).toBe(true);
    expect(matches("*.generated.ts", "packages/x/src/a.ts")).toBe(false);
  });

  it("does not read a dot or a plus as a wildcard", () => {
    expect(matches("a.ts", "aXts")).toBe(false);
    expect(matches("c++/x.ts", "c++/x.ts")).toBe(true);
    expect(matchesAny(["./packages/api"], "packages/api/src/a.ts")).toBe(true);
  });
});

describe("a monorepo's config", () => {
  it("leaves ignored paths out of the report and the exit code, and says how many", () => {
    const repo = monorepo();
    config(repo, { ignore: ["packages/generated", "packages/legacy/**"] });
    const { out, code } = run(["review", "--all", "--fail-on", "low", ...NO_TOOLS], repo);
    expect(code).toBe(2);
    expect(out).toContain("packages/api/src/a.ts");
    expect(out).toContain("packages/web/src/w.ts");
    expect(out).not.toContain("packages/generated");
    expect(out).not.toContain("packages/legacy");
    expect(out).toContain("2 findings under paths the config ignores are left out");

    const json = JSON.parse(run(["--json", "review", "--all", ...NO_TOOLS], repo).out);
    expect(json.reviewResult.findings).toHaveLength(2);
    expect(json.reviewResult.summary.totalFindings).toBe(2);
  });

  it("passes when everything found is ignored", () => {
    const repo = monorepo();
    config(repo, { ignore: ["packages/**"] });
    expect(run(["review", "--all", "--fail-on", "low", ...NO_TOOLS], repo).code).toBe(0);
  });

  it("holds one package to a different bar than the rest", () => {
    const repo = monorepo();
    // `var` is low. The API must be clean at low; everything else may pass up to high.
    config(repo, {
      review: { failOn: "high" },
      overrides: [{ paths: ["packages/api"], failOn: "low" }],
    });
    expect(run(["review", "--all", ...NO_TOOLS], repo).code).toBe(2);

    // Take the API's finding away: the rest is below the repository's bar.
    write(repo, { "packages/api/src/a.ts": "export const api = 1;\n" });
    expect(run(["review", "--all", ...NO_TOOLS], repo).code).toBe(0);
  });

  it("an override can also relax a package below the repository's bar", () => {
    const repo = monorepo();
    config(repo, {
      review: { failOn: "low" },
      overrides: [{ paths: ["packages/legacy", "packages/generated"], failOn: "none" }],
    });
    write(repo, { "packages/api/src/a.ts": "export const api = 1;\n" });
    write(repo, { "packages/web/src/w.ts": "export const web = 1;\n" });
    // Only legacy and generated have findings left, and they are exempt.
    const { code, out } = run(["review", "--all", ...NO_TOOLS], repo);
    expect(code).toBe(0);
    expect(out).toContain("packages/legacy/src/l.ts"); // still reported
  });

  it("the first matching override wins", () => {
    const repo = monorepo();
    config(repo, {
      review: { failOn: "none" },
      overrides: [
        { paths: ["packages/api/src"], failOn: "none" },
        { paths: ["packages/api"], failOn: "low" },
      ],
    });
    expect(run(["review", "--all", ...NO_TOOLS], repo).code).toBe(0);
  });

  it("patterns are written from the config's directory, even when the review runs inside a package", () => {
    const repo = monorepo();
    config(repo, {
      ignore: ["packages/api/**"],
      overrides: [{ paths: ["packages/web"], failOn: "none" }],
    });
    const api = run(
      ["-C", join(repo, "packages/api"), "review", "--all", "--fail-on", "low", ...NO_TOOLS],
      repo,
    );
    expect(api.code).toBe(0); // its only finding is under an ignored path
    expect(api.out).not.toContain("a.ts");

    const web = run(
      ["-C", join(repo, "packages/web"), "review", "--all", "--fail-on", "low", ...NO_TOOLS],
      repo,
    );
    expect(web.code).toBe(0); // exempt by override
    expect(web.out).toContain("src/w.ts"); // but still reported, relative to where it ran

    const legacy = run(
      ["-C", join(repo, "packages/legacy"), "review", "--all", "--fail-on", "low", ...NO_TOOLS],
      repo,
    );
    expect(legacy.code).toBe(2);
  });

  it("combines with reviewing only the change", () => {
    const repo = monorepo();
    config(repo, {
      ignore: ["packages/generated"],
      overrides: [{ paths: ["packages/api"], failOn: "low" }],
    });
    write(repo, {
      "packages/web/src/w.ts": "var web = 1;\nvar more = 2;\n",
      "packages/generated/src/g.ts": "var generated = 1;\nvar generatedToo = 2;\n",
    });
    const { out, code } = run(
      ["review", "--changed-since", "HEAD", "--fail-on-new", "--fail-on", "low", ...NO_TOOLS],
      repo,
    );
    expect(code).toBe(2);
    expect(out).toContain("packages/web/src/w.ts:2");
    expect(out).not.toContain("packages/generated");
  });

  it("rejects an override that names no path or a bar that does not exist", () => {
    const repo = monorepo();
    config(repo, { overrides: [{ paths: [], failOn: "low" }] });
    expect(run(["review", "--all", ...NO_TOOLS], repo).code).toBe(1);
    config(repo, { overrides: [{ paths: ["packages/api"], failOn: "severe" }] });
    const bad = run(["review", "--all", ...NO_TOOLS], repo);
    expect(bad.code).toBe(1);
    expect(bad.err + bad.out).toContain("overrides");
    config(repo, { overrides: [{ paths: ["packages/api"], failOn: "low", note: "x" }] });
    expect(run(["review", "--all", ...NO_TOOLS], repo).err).toContain("is not a setting");
  });
});

describe("GitLab", () => {
  const parse = (yaml: string) => Bun.YAML.parse(yaml) as Record<string, any>;

  it("both generated pipelines are valid YAML with the pieces CI needs", () => {
    const github = parse(githubWorkflow("high", "1.2.3"));
    expect(github.on).toEqual({ pull_request: null });
    expect(github.jobs.review.steps.length).toBe(5);

    const gitlab = parse(gitlabPipeline("high", "1.2.3"));
    const job = gitlab.debuggatha;
    expect(job.image).toBe("node:22");
    expect(job.rules[0].if).toContain("merge_request_event");
    expect(job.variables.GIT_DEPTH).toBe("0");
    expect(job.artifacts).toEqual({
      when: "always",
      reports: { codequality: "gl-code-quality-report.json" },
    });
    expect(job.script.join("\n")).toContain("@sxnnyside/debuggatha-cli@1.2.3");
    expect(job.script.join("\n")).toContain("--fail-on high");
  });

  it("init writes .gitlab-ci.yml when the repository has no pipeline", () => {
    const repo = monorepo();
    const { out } = run(["init", "--ci", "gitlab"], repo);
    expect(out).toContain("Created .gitlab-ci.yml");
    expect(parse(readFileSync(join(repo, ".gitlab-ci.yml"), "utf8")).debuggatha).toBeDefined();
    expect(existsSync(join(repo, ".github"))).toBe(false);
  });

  it("never edits a pipeline that is there: the job goes in its own file, with how to include it", () => {
    const repo = monorepo();
    write(repo, { ".gitlab-ci.yml": "stages: [build]\nbuild:\n  script: echo hi\n" });
    const { out } = run(["init", "--ci", "gitlab"], repo);
    expect(readFileSync(join(repo, ".gitlab-ci.yml"), "utf8")).toBe(
      "stages: [build]\nbuild:\n  script: echo hi\n",
    );
    expect(existsSync(join(repo, ".gitlab/debuggatha.gitlab-ci.yml"))).toBe(true);
    expect(out).toContain("include:");
    expect(out).toContain("local: .gitlab/debuggatha.gitlab-ci.yml");
  });

  it("picks GitLab by itself from the remote, and GitHub otherwise", () => {
    const gitlabRepo = monorepo();
    git(gitlabRepo, "remote", "add", "origin", "git@gitlab.com:acme/app.git");
    run(["init"], gitlabRepo);
    expect(existsSync(join(gitlabRepo, ".gitlab-ci.yml"))).toBe(true);
    expect(existsSync(join(gitlabRepo, ".github"))).toBe(false);

    const githubRepo = monorepo();
    git(githubRepo, "remote", "add", "origin", "https://github.com/acme/app.git");
    run(["init"], githubRepo);
    expect(existsSync(join(githubRepo, ".github/workflows/debuggatha.yml"))).toBe(true);
    expect(existsSync(join(githubRepo, ".gitlab-ci.yml"))).toBe(false);
  });

  it("the Code Quality report has what GitLab reads, with a fingerprint that tells findings apart", () => {
    const repo = monorepo();
    write(repo, {
      "packages/api/src/a.ts": 'var a = 1;\nvar b = 2;\nif (a == b) { eval("1"); }\n',
    });
    const report = JSON.parse(
      run(["review", "--all", "--format", "gitlab", "--fail-on", "none", ...NO_TOOLS], repo).out,
    );
    expect(Array.isArray(report)).toBe(true);
    const evalIssue = report.find(
      (issue: { check_name: string }) => issue.check_name === "no-eval",
    );
    expect(evalIssue).toMatchObject({
      severity: "critical", // high in Debuggatha, critical in GitLab's scale
      location: { path: "packages/api/src/a.ts", lines: { begin: 3 } },
    });
    expect(evalIssue.description.length).toBeGreaterThan(0);
    const fingerprints = report.map((issue: { fingerprint: string }) => issue.fingerprint);
    expect(new Set(fingerprints).size).toBe(fingerprints.length);
    expect(fingerprints.every((fp: string) => /^[0-9a-f]{32}$/.test(fp))).toBe(true);

    // The same code gives the same fingerprints, so GitLab can tell what is new.
    const again = JSON.parse(
      run(["review", "--all", "--format", "gitlab", "--fail-on", "none", ...NO_TOOLS], repo).out,
    );
    expect(again.map((issue: { fingerprint: string }) => issue.fingerprint)).toEqual(fingerprints);
  });

  it("no findings is an empty report, and nothing changed is too", () => {
    const repo = monorepo();
    write(repo, { "packages/api/src/a.ts": "export const a = 1;\n" });
    git(repo, "add", "-A");
    git(repo, "commit", "-q", "-m", "clean");
    config(repo, { ignore: ["packages/**"] });
    expect(
      JSON.parse(run(["review", "--all", "--format", "gitlab", ...NO_TOOLS], repo).out),
    ).toEqual([]);
    expect(
      JSON.parse(
        run(["review", "--changed-since", "HEAD", "--format", "gitlab", ...NO_TOOLS], repo).out,
      ),
    ).toEqual([]);
  });

  it("the job's own command, run for real, fails on what the merge request introduced and leaves the report", () => {
    const repo = monorepo();
    git(repo, "checkout", "-q", "-b", "feature");
    write(repo, {
      "packages/web/src/w.ts": "var web = 1;\nexport const run = (s: string) => eval(s);\n",
    });
    git(repo, "add", "-A");
    git(repo, "commit", "-q", "-m", "risky");

    const job = parseJob(gitlabPipeline("high", "0.0.0"));
    const reportPath = join(repo, "gl-code-quality-report.json");
    const { code } = run(job.reviewArgs("main", reportPath), repo);
    expect(code).toBe(2);
    const report = JSON.parse(readFileSync(reportPath, "utf8"));
    expect(report.map((issue: { check_name: string }) => issue.check_name)).toContain("no-eval");
    // The old `var` on line 1 is not the merge request's, so it is not in what fails the job...
    // ...but it is still in the report for people to see.
    expect(
      report.some(
        (issue: { location: { lines: { begin: number } } }) => issue.location.lines.begin === 1,
      ),
    ).toBe(true);
  });

  /** The review command from the pipeline's `script`, as arguments, with the CI variables filled in. */
  function parseJob(yaml: string) {
    const script = (parse(yaml).debuggatha.script as string[]).find((line) =>
      line.includes(" review "),
    );
    expect(script).toBeDefined();
    return {
      reviewArgs(target: string, output: string) {
        const rest = (script as string)
          .replace(/^npx --yes @sxnnyside\/debuggatha-cli@\S+ /, "")
          .replace('"origin/$CI_MERGE_REQUEST_TARGET_BRANCH_NAME"', target)
          .replace("gl-code-quality-report.json", output);
        return rest.split(/\s+/).filter(Boolean);
      },
    };
  }
});
