import { execFileSync } from "node:child_process";
import { chmodSync, existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { type Command, Option } from "commander";
import pkg from "../../package.json" with { type: "json" };
import { CONFIG_FILE } from "../config.js";
import { SEVERITY_ORDER } from "../report.js";
import type { CliLogger } from "../utils/logger.js";

const HOOK_MARKER = "# debuggatha-hook";

interface InitOptions {
  ci?: "github" | "gitlab" | "none";
  hooks?: boolean;
  failOn: string;
  force?: boolean;
}

type Outcome = "created" | "updated" | "kept";
interface Step {
  file: string;
  outcome: Outcome;
  note?: string;
}

/**
 * The workflow every new repository gets: it reviews only what the pull request changes and
 * fails only on findings on the lines it touched, so adopting Debuggatha never means fixing
 * the whole repository first. The comment step is best effort: a pull request from a fork
 * has a read-only token and cannot comment, and that must not fail the check.
 */
export function githubWorkflow(failOn: string, version: string): string {
  return `# Written by \`debuggatha init\`. Safe to edit.
name: Debuggatha

on:
  pull_request:

permissions:
  contents: read
  pull-requests: write

jobs:
  review:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0
      - uses: actions/setup-node@v4
        with:
          node-version: 22
      - name: Review what this pull request changes
        id: review
        run: |
          set +e
          npx --yes @debuggatha/cli@${version} review \\
            --changed-since "origin/\${{ github.base_ref }}" \\
            --fail-on-new --fail-on ${failOn} --no-persist \\
            --format markdown --output debuggatha-report.md
          echo "code=$?" >> "$GITHUB_OUTPUT"
      - name: Comment on the pull request
        if: always() && hashFiles('debuggatha-report.md') != ''
        continue-on-error: true
        uses: actions/github-script@v7
        with:
          script: |
            const fs = require("node:fs");
            const body = fs.readFileSync("debuggatha-report.md", "utf8");
            const marker = "<!-- debuggatha-report -->";
            const { owner, repo } = context.repo;
            const issue_number = context.issue.number;
            const comments = await github.paginate(github.rest.issues.listComments, { owner, repo, issue_number });
            const mine = comments.find((comment) => comment.body?.includes(marker));
            if (mine) await github.rest.issues.updateComment({ owner, repo, comment_id: mine.id, body });
            else await github.rest.issues.createComment({ owner, repo, issue_number, body });
      - name: Fail if the change introduced findings
        if: steps.review.outputs.code != '0'
        run: exit \${{ steps.review.outputs.code }}
`;
}

/**
 * The GitLab equivalent: a merge-request job that reviews what the merge request changes and
 * publishes a Code Quality report, which GitLab shows as a widget on the merge request. The
 * report is kept even when the job fails (`when: always`), and `after_script` prints the
 * findings in the job log, since the report itself is JSON for GitLab, not for people.
 */
export function gitlabPipeline(failOn: string, version: string): string {
  const review = `npx --yes @debuggatha/cli@${version} review --changed-since "origin/$CI_MERGE_REQUEST_TARGET_BRANCH_NAME" --fail-on-new --no-persist`;
  return `# Written by \`debuggatha init\`. Safe to edit.
debuggatha:
  image: node:22
  rules:
    - if: $CI_PIPELINE_SOURCE == "merge_request_event"
  variables:
    GIT_DEPTH: "0"
  script:
    - git fetch --quiet origin "$CI_MERGE_REQUEST_TARGET_BRANCH_NAME"
    - ${review} --fail-on ${failOn} --format gitlab --output gl-code-quality-report.json
  after_script:
    - git fetch --quiet origin "$CI_MERGE_REQUEST_TARGET_BRANCH_NAME" || true
    - ${review} --fail-on none || true
  artifacts:
    when: always
    reports:
      codequality: gl-code-quality-report.json
`;
}

/** The provider the repository lives on, judged from its remote; GitHub when it cannot tell. */
export function detectCiProvider(cwd: string): "github" | "gitlab" {
  if (existsSync(join(cwd, ".gitlab-ci.yml")) && !existsSync(join(cwd, ".github"))) return "gitlab";
  try {
    const remote = execFileSync("git", ["remote", "get-url", "origin"], {
      cwd,
      encoding: "utf8",
      stdio: ["ignore", "pipe", "ignore"],
    });
    if (/gitlab/i.test(remote)) return "gitlab";
  } catch {
    // no remote
  }
  return "github";
}

export function preCommitHook(failOn: string): string {
  return `#!/bin/sh
${HOOK_MARKER}
# Reviews what you are about to commit; fails only on findings on the lines you changed.
# It never downloads anything: without the CLI installed it says so and lets the commit through.
if [ -x node_modules/.bin/debuggatha ]; then
  exec node_modules/.bin/debuggatha review --fail-on-new --fail-on ${failOn} --no-persist --quiet
fi
if command -v debuggatha >/dev/null 2>&1; then
  exec debuggatha review --fail-on-new --fail-on ${failOn} --no-persist --quiet
fi
echo "debuggatha: not installed here, so the pre-commit review was skipped (npm install --save-dev @debuggatha/cli)." >&2
exit 0
`;
}

/** Which kind of monorepo this is, when it is one. */
function workspaceKinds(cwd: string): string[] {
  const kinds: string[] = [];
  try {
    const manifest = JSON.parse(readFileSync(join(cwd, "package.json"), "utf8")) as {
      workspaces?: unknown;
    };
    if (manifest.workspaces) kinds.push("package.json workspaces");
  } catch {
    // no package.json
  }
  if (existsSync(join(cwd, "pnpm-workspace.yaml"))) kinds.push("pnpm workspace");
  if (existsSync(join(cwd, "go.work"))) kinds.push("Go workspace");
  try {
    if (/^\[workspace\]/m.test(readFileSync(join(cwd, "Cargo.toml"), "utf8"))) {
      kinds.push("Cargo workspace");
    }
  } catch {
    // no Cargo.toml
  }
  if (existsSync(join(cwd, "settings.gradle")) || existsSync(join(cwd, "settings.gradle.kts"))) {
    kinds.push("Gradle multi-project build");
  }
  return kinds;
}

function write(path: string, content: string, force: boolean): Outcome {
  const existed = existsSync(path);
  if (existed && !force) return "kept";
  mkdirSync(dirname(path), { recursive: true });
  writeFileSync(path, content);
  return existed ? "updated" : "created";
}

/** `core.hooksPath` means another tool (husky, lefthook) owns the hooks; writing into `.git/hooks` would do nothing. */
function hooksDirectory(cwd: string): { dir: string } | { reason: string } {
  try {
    const custom = execFileSync("git", ["config", "--get", "core.hooksPath"], {
      cwd,
      encoding: "utf8",
      stdio: ["ignore", "pipe", "ignore"],
    }).trim();
    if (custom) {
      return {
        reason: `git is configured to use hooks from "${custom}" (a hook manager owns them). Add this to its pre-commit: debuggatha review --fail-on-new --fail-on high --no-persist --quiet`,
      };
    }
  } catch {
    // no core.hooksPath set
  }
  try {
    const gitDir = execFileSync("git", ["rev-parse", "--git-path", "hooks"], {
      cwd,
      encoding: "utf8",
      stdio: ["ignore", "pipe", "ignore"],
    }).trim();
    return { dir: join(cwd, gitDir) };
  } catch {
    return { reason: "this is not a git repository, so there is nowhere to install a hook." };
  }
}

export function registerInitCommand(program: Command) {
  program
    .command("init")
    .description(
      "Set a repository up: a config file, a CI workflow that reviews only what a pull request changes, and optionally a pre-commit hook",
    )
    .addOption(
      new Option(
        "--ci <provider>",
        "Write a CI workflow for this provider (default: the one the repository's remote points at)",
      ).choices(["github", "gitlab", "none"]),
    )
    .addOption(
      new Option("--fail-on <severity>", "Lowest severity that fails CI and the hook")
        .choices(SEVERITY_ORDER.filter((severity) => severity !== "informational"))
        .default("high"),
    )
    .option("--hooks", "Install a git pre-commit hook that reviews what you are about to commit")
    .option("--force", "Overwrite files init wrote before")
    .action((options: InitOptions, command) => {
      const logger = command.logger as CliLogger;
      const cwd = program.opts().cwd as string;
      const steps: Step[] = [];

      try {
        steps.push({
          file: CONFIG_FILE,
          outcome: write(
            join(cwd, CONFIG_FILE),
            `${JSON.stringify({ review: { failOn: options.failOn }, analyzers: { mode: "auto" } }, null, 2)}\n`,
            Boolean(options.force),
          ),
        });

        const ci = options.ci ?? detectCiProvider(cwd);
        if (ci === "gitlab") {
          const hasPipeline = existsSync(join(cwd, ".gitlab-ci.yml"));
          // An existing pipeline is the team's; the job goes in its own file, for them to include.
          const file = hasPipeline ? join(".gitlab", "debuggatha.gitlab-ci.yml") : ".gitlab-ci.yml";
          steps.push({
            file,
            outcome: write(
              join(cwd, file),
              gitlabPipeline(options.failOn, pkg.version),
              Boolean(options.force),
            ),
            ...(hasPipeline
              ? {
                  note: `add it to your .gitlab-ci.yml with:\n  include:\n    - local: .gitlab/debuggatha.gitlab-ci.yml`,
                }
              : {}),
          });
        }
        if (ci === "github") {
          const file = join(".github", "workflows", "debuggatha.yml");
          steps.push({
            file,
            outcome: write(
              join(cwd, file),
              githubWorkflow(options.failOn, pkg.version),
              Boolean(options.force),
            ),
          });
        }

        if (options.hooks) {
          const hooks = hooksDirectory(cwd);
          if ("reason" in hooks) {
            steps.push({ file: "pre-commit hook", outcome: "kept", note: hooks.reason });
          } else {
            const path = join(hooks.dir, "pre-commit");
            const ours = existsSync(path) && readFileSync(path, "utf8").includes(HOOK_MARKER);
            if (existsSync(path) && !ours && !options.force) {
              steps.push({
                file: path,
                outcome: "kept",
                note: "a pre-commit hook that Debuggatha did not write is already there; --force replaces it",
              });
            } else {
              write(path, preCommitHook(options.failOn), true);
              chmodSync(path, 0o755);
              steps.push({ file: path, outcome: ours ? "updated" : "created" });
            }
          }
        }

        const monorepo = workspaceKinds(cwd);
        if (logger.isJson) {
          logger.json({ status: "success", steps, monorepo });
          return;
        }
        for (const step of steps) {
          const verb =
            step.outcome === "kept" ? "Kept" : step.outcome === "updated" ? "Updated" : "Created";
          const line = `${verb} ${step.file}${step.note ? `: ${step.note}` : "."}`;
          if (step.outcome === "kept") logger.info(line);
          else logger.success(line);
        }
        if (monorepo.length > 0) {
          logger.info(
            `This looks like a monorepo (${monorepo.join(", ")}). CI reviews only the files a pull request changes, so it scales with the change rather than the repository.`,
          );
        }
        logger.info(
          "Next: commit these files. To accept the findings the repository has today, run `debuggatha baseline create` and commit .debuggatha/baseline.json. Keep .debuggatha/ledger.json out of version control if you do not want your local history shared.",
        );
      } catch (err) {
        logger.error("Failed to initialize Debuggatha.", err);
        process.exit(1);
      }
    });
}
