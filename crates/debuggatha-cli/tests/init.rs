//! `debuggatha init`, run as the binary.
#![allow(clippy::expect_used, clippy::unwrap_used)] // a failed expect is a failed test

use std::path::PathBuf;
use std::process::{Command, Output};

struct Repo(PathBuf);

impl Repo {
    fn new(name: &str) -> Self {
        let dir =
            std::env::temp_dir().join(format!("debuggatha-init-{name}-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }

    fn init(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_debuggatha"))
            .args(["init", "--root"])
            .arg(&self.0)
            .args(args)
            .output()
            .unwrap()
    }

    fn git_init(&self) {
        assert!(
            Command::new("git")
                .args(["init", "-q", "-b", "main"])
                .current_dir(&self.0)
                .status()
                .unwrap()
                .success()
        );
    }
}

impl Drop for Repo {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).ok();
    }
}

fn text(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

#[test]
fn it_writes_the_config_and_a_workflow_and_keeps_what_exists() {
    let repo = Repo::new("github");
    let out = repo.init(&[]);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out));
    let config: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(repo.0.join(".debuggatha/config.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(config["review"]["failOn"], "high");
    let workflow =
        std::fs::read_to_string(repo.0.join(".github/workflows/debuggatha.yml")).unwrap();
    assert!(workflow.contains("--changed-since \"origin/${{ github.base_ref }}\""));
    assert!(workflow.contains("--fail-on high"));
    assert!(workflow.contains(&format!("debuggatha-cli@{}", env!("CARGO_PKG_VERSION"))));

    std::fs::write(repo.0.join(".github/workflows/debuggatha.yml"), "mine").unwrap();
    assert!(text(&repo.init(&[])).contains("Kept .github/workflows/debuggatha.yml"));
    assert_eq!(
        std::fs::read_to_string(repo.0.join(".github/workflows/debuggatha.yml")).unwrap(),
        "mine"
    );
    assert!(text(&repo.init(&["--force", "--fail-on", "critical"])).contains("Updated"));
    assert!(
        std::fs::read_to_string(repo.0.join(".github/workflows/debuggatha.yml"))
            .unwrap()
            .contains("--fail-on critical")
    );
}

#[test]
fn gitlab_keeps_the_teams_own_pipeline() {
    let repo = Repo::new("gitlab");
    std::fs::write(repo.0.join(".gitlab-ci.yml"), "stages: [test]\n").unwrap();
    let out = repo.init(&[]);
    assert!(
        text(&out).contains(".gitlab/debuggatha.gitlab-ci.yml"),
        "{}",
        text(&out)
    );
    assert_eq!(
        std::fs::read_to_string(repo.0.join(".gitlab-ci.yml")).unwrap(),
        "stages: [test]\n"
    );
    assert!(
        std::fs::read_to_string(repo.0.join(".gitlab/debuggatha.gitlab-ci.yml"))
            .unwrap()
            .contains("--format gitlab")
    );
    assert!(!repo.0.join(".github").exists());
}

#[test]
fn a_hook_is_installed_only_where_nobody_else_owns_it() {
    let repo = Repo::new("hook");
    assert!(text(&repo.init(&["--ci", "none", "--hooks"])).contains("not a git repository"));

    repo.git_init();
    assert!(text(&repo.init(&["--ci", "none", "--hooks"])).contains("Created pre-commit hook"));
    let hook = repo.0.join(".git/hooks/pre-commit");
    assert!(
        std::fs::read_to_string(&hook)
            .unwrap()
            .contains("# debuggatha-hook")
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_ne!(
            std::fs::metadata(&hook).unwrap().permissions().mode() & 0o111,
            0
        );
    }
    assert!(text(&repo.init(&["--ci", "none", "--hooks"])).contains("Updated pre-commit hook"));

    std::fs::write(&hook, "#!/bin/sh\necho theirs\n").unwrap();
    assert!(text(&repo.init(&["--ci", "none", "--hooks"])).contains("did not write"));
    assert!(std::fs::read_to_string(&hook).unwrap().contains("theirs"));
}

#[test]
fn it_says_when_the_repository_is_a_monorepo() {
    let repo = Repo::new("mono");
    std::fs::write(repo.0.join("package.json"), r#"{"workspaces":["a"]}"#).unwrap();
    std::fs::write(repo.0.join("pnpm-workspace.yaml"), "packages: []\n").unwrap();
    let out = text(&repo.init(&["--ci", "none"]));
    assert!(
        out.contains("package.json workspaces, pnpm workspace"),
        "{out}"
    );
}
