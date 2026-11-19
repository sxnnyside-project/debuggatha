//! `debuggatha init`: a config file, a CI workflow that reviews only what a pull request changes,
//! and optionally a pre-commit hook.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use clap::{Args as ClapArgs, ValueEnum};
use serde::Serialize;
use serde_json::json;

use crate::Context;
use crate::config::CONFIG_FILE;
use crate::report::Format;

const HOOK_MARKER: &str = "# debuggatha-hook";

#[derive(Clone, Copy, ValueEnum)]
enum Ci {
    Github,
    Gitlab,
    None,
}

#[derive(ClapArgs)]
pub(crate) struct Args {
    /// Write a CI workflow for this provider (default: the one the remote points at).
    #[arg(long, value_enum)]
    ci: Option<Ci>,
    /// Lowest severity that fails CI and the hook.
    #[arg(long, default_value = "high", value_parser = clap::builder::PossibleValuesParser::new(["low", "medium", "high", "critical"]))]
    fail_on: String,
    /// Install a git pre-commit hook that reviews what you are about to commit.
    #[arg(long)]
    hooks: bool,
    /// Overwrite files init wrote before.
    #[arg(long)]
    force: bool,
}

#[derive(Serialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum Outcome {
    Created,
    Updated,
    Kept,
}

#[derive(Serialize)]
struct Step {
    file: String,
    outcome: Outcome,
    #[serde(skip_serializing_if = "Option::is_none")]
    note: Option<String>,
}

/// The workflow every new repository gets: it reviews only what the pull request changes and fails
/// only on findings on the lines it touched, so adopting Debuggatha never means fixing the whole
/// repository first. The comment step is best effort: a pull request from a fork has a read-only
/// token and cannot comment, and that must not fail the check.
fn github_workflow(fail_on: &str, version: &str) -> String {
    r#"# Written by `debuggatha init`. Safe to edit.
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
          npx --yes @sxnnyside/debuggatha-cli@@VERSION@ review \
            --changed-since "origin/${{ github.base_ref }}" \
            --fail-on-new --fail-on @FAIL_ON@ --no-persist \
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
        run: exit ${{ steps.review.outputs.code }}
"#
    .replace("@VERSION@", version)
    .replace("@FAIL_ON@", fail_on)
}

/// The GitLab equivalent: a merge-request job that publishes a Code Quality report, which GitLab
/// shows as a widget. The report is kept even when the job fails, and `after_script` prints the
/// findings in the job log, since the report itself is JSON for GitLab, not for people.
fn gitlab_pipeline(fail_on: &str, version: &str) -> String {
    let review = format!(
        "npx --yes @sxnnyside/debuggatha-cli@{version} review --changed-since \"origin/$CI_MERGE_REQUEST_TARGET_BRANCH_NAME\" --fail-on-new --no-persist"
    );
    format!(
        r#"# Written by `debuggatha init`. Safe to edit.
debuggatha:
  image: node:22
  rules:
    - if: $CI_PIPELINE_SOURCE == "merge_request_event"
  variables:
    GIT_DEPTH: "0"
  script:
    - git fetch --quiet origin "$CI_MERGE_REQUEST_TARGET_BRANCH_NAME"
    - {review} --fail-on {fail_on} --format gitlab --output gl-code-quality-report.json
  after_script:
    - git fetch --quiet origin "$CI_MERGE_REQUEST_TARGET_BRANCH_NAME" || true
    - {review} --fail-on none || true
  artifacts:
    when: always
    reports:
      codequality: gl-code-quality-report.json
"#
    )
}

fn pre_commit_hook(fail_on: &str) -> String {
    format!(
        r#"#!/bin/sh
{HOOK_MARKER}
# Reviews what you are about to commit; fails only on findings on the lines you changed.
# It never downloads anything: without the CLI installed it says so and lets the commit through.
if [ -x node_modules/.bin/debuggatha ]; then
  exec node_modules/.bin/debuggatha review --fail-on-new --fail-on {fail_on} --no-persist --quiet
fi
if command -v debuggatha >/dev/null 2>&1; then
  exec debuggatha review --fail-on-new --fail-on {fail_on} --no-persist --quiet
fi
echo "debuggatha: not installed here, so the pre-commit review was skipped (npm install --save-dev @sxnnyside/debuggatha-cli)." >&2
exit 0
"#
    )
}

fn git_output(root: &Path, args: &[&str]) -> Option<String> {
    let out = Command::new("git")
        .args(args)
        .current_dir(root)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_owned())
}

/// The provider the repository lives on, judged from its remote; GitHub when it cannot tell.
fn detect_ci(root: &Path) -> Ci {
    if root.join(".gitlab-ci.yml").exists() && !root.join(".github").exists() {
        return Ci::Gitlab;
    }
    match git_output(root, &["remote", "get-url", "origin"]) {
        Some(remote) if remote.to_lowercase().contains("gitlab") => Ci::Gitlab,
        _ => Ci::Github,
    }
}

/// Which kinds of monorepo this is, when it is one.
fn workspace_kinds(root: &Path) -> Vec<&'static str> {
    let mut kinds = Vec::new();
    let read = |name: &str| std::fs::read_to_string(root.join(name)).ok();
    if read("package.json")
        .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
        .is_some_and(|v| v.get("workspaces").is_some_and(|w| !w.is_null()))
    {
        kinds.push("package.json workspaces");
    }
    if root.join("pnpm-workspace.yaml").exists() {
        kinds.push("pnpm workspace");
    }
    if root.join("go.work").exists() {
        kinds.push("Go workspace");
    }
    if read("Cargo.toml").is_some_and(|t| t.lines().any(|l| l.starts_with("[workspace]"))) {
        kinds.push("Cargo workspace");
    }
    if root.join("settings.gradle").exists() || root.join("settings.gradle.kts").exists() {
        kinds.push("Gradle multi-project build");
    }
    kinds
}

fn write(path: &Path, content: &str, force: bool) -> Result<Outcome, String> {
    let existed = path.exists();
    if existed && !force {
        return Ok(Outcome::Kept);
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|_| "A directory could not be created.".to_owned())?;
    }
    std::fs::write(path, content).map_err(|_| "A file could not be written.".to_owned())?;
    Ok(if existed {
        Outcome::Updated
    } else {
        Outcome::Created
    })
}

/// `core.hooksPath` means another tool (husky, lefthook) owns the hooks; writing into
/// `.git/hooks` would do nothing.
fn hooks_directory(root: &Path) -> Result<PathBuf, String> {
    if let Some(custom) =
        git_output(root, &["config", "--get", "core.hooksPath"]).filter(|c| !c.is_empty())
    {
        return Err(format!(
            "git is configured to use hooks from \"{custom}\" (a hook manager owns them). Add this to its pre-commit: debuggatha review --fail-on-new --fail-on high --no-persist --quiet"
        ));
    }
    git_output(root, &["rev-parse", "--git-path", "hooks"])
        .map(|dir| root.join(dir))
        .ok_or_else(|| {
            "this is not a git repository, so there is nowhere to install a hook.".to_owned()
        })
}

fn install_hook(root: &Path, args: &Args) -> Result<Step, String> {
    let name = "pre-commit hook".to_owned();
    let dir = match hooks_directory(root) {
        Ok(dir) => dir,
        Err(reason) => {
            return Ok(Step {
                file: name,
                outcome: Outcome::Kept,
                note: Some(reason),
            });
        }
    };
    let path = dir.join("pre-commit");
    let ours = std::fs::read_to_string(&path).is_ok_and(|t| t.contains(HOOK_MARKER));
    if path.exists() && !ours && !args.force {
        return Ok(Step {
            file: name,
            outcome: Outcome::Kept,
            note: Some("a pre-commit hook that Debuggatha did not write is already there; --force replaces it".to_owned()),
        });
    }
    write(&path, &pre_commit_hook(&args.fail_on), true)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
            .map_err(|_| "The hook could not be made executable.".to_owned())?;
    }
    Ok(Step {
        file: name,
        outcome: if ours {
            Outcome::Updated
        } else {
            Outcome::Created
        },
        note: None,
    })
}

pub(crate) fn run(context: &Context, args: &Args) -> Result<bool, String> {
    let root = context.root()?;
    let version = debuggatha_engine::VERSION;
    let mut steps = Vec::new();

    let config = json!({ "review": { "failOn": args.fail_on }, "analyzers": { "mode": "auto" } });
    steps.push(Step {
        file: CONFIG_FILE.to_owned(),
        outcome: write(
            &root.join(CONFIG_FILE),
            &format!(
                "{}\n",
                serde_json::to_string_pretty(&config).unwrap_or_default()
            ),
            args.force,
        )?,
        note: None,
    });

    match args.ci.unwrap_or_else(|| detect_ci(&root)) {
        Ci::Gitlab => {
            let has_pipeline = root.join(".gitlab-ci.yml").exists();
            // An existing pipeline is the team's; the job goes in its own file, for them to include.
            let file = if has_pipeline {
                ".gitlab/debuggatha.gitlab-ci.yml"
            } else {
                ".gitlab-ci.yml"
            };
            steps.push(Step {
                file: file.to_owned(),
                outcome: write(&root.join(file), &gitlab_pipeline(&args.fail_on, version), args.force)?,
                note: has_pipeline.then(|| "add it to your .gitlab-ci.yml with:\n  include:\n    - local: .gitlab/debuggatha.gitlab-ci.yml".to_owned()),
            });
        }
        Ci::Github => {
            let file = ".github/workflows/debuggatha.yml";
            steps.push(Step {
                file: file.to_owned(),
                outcome: write(
                    &root.join(file),
                    &github_workflow(&args.fail_on, version),
                    args.force,
                )?,
                note: None,
            });
        }
        Ci::None => {}
    }
    if args.hooks {
        steps.push(install_hook(&root, args)?);
    }

    let monorepo = workspace_kinds(&root);
    match context.format() {
        Format::Json => println!(
            "{}",
            serde_json::to_string_pretty(
                &json!({ "status": "success", "steps": steps, "monorepo": monorepo })
            )
            .unwrap_or_default()
        ),
        Format::Text => {
            for step in &steps {
                let verb = match step.outcome {
                    Outcome::Kept => "Kept",
                    Outcome::Updated => "Updated",
                    Outcome::Created => "Created",
                };
                match &step.note {
                    Some(note) => println!("{verb} {}: {note}", step.file),
                    None => println!("{verb} {}.", step.file),
                }
            }
            if !monorepo.is_empty() {
                println!(
                    "This looks like a monorepo ({}). CI reviews only the files a pull request changes, so it scales with the change rather than the repository.",
                    monorepo.join(", ")
                );
            }
            println!(
                "Next: commit these files. To accept the findings the repository has today, run `debuggatha baseline create` and commit .debuggatha/baseline.json. Keep .debuggatha/ledger.json out of version control if you do not want your local history shared."
            );
        }
    }
    Ok(false)
}
