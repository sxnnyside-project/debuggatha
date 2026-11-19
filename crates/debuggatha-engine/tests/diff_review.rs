//! Diff review against a real git repository: only the lines a change added are findings, files
//! git does not track are read whole, and what could not be reviewed is returned.
#![allow(clippy::expect_used, clippy::unwrap_used)] // a failed expect is a failed test

use std::path::{Path, PathBuf};
use std::process::Command;

use debuggatha_core::env::Env;
use debuggatha_core::knowledge_system::ReviewPolicy;
use debuggatha_engine::detect::coverage::SkipReason;
use debuggatha_engine::detect::default_registry;
use debuggatha_engine::diff_review::{DiffReviewError, review_diff};
use debuggatha_engine::git::GitError;
use serde_json::json;

struct FixedEnv;
impl Env for FixedEnv {
    fn now(&self) -> String {
        "2026-01-01T00:00:00.000Z".into()
    }
    fn new_id(&self) -> String {
        "id".into()
    }
}

fn policy() -> ReviewPolicy {
    serde_json::from_value(json!({
        "id": "p",
        "rules": [{
            "id": "no-eval",
            "statement": "Do not evaluate strings as code.",
            "origin": { "kind": "pack", "packId": "debuggatha/owasp", "packVersion": "1.0.0" }
        }],
        "packRefs": [], "conflicts": []
    }))
    .expect("policy")
}

struct Repo(PathBuf);

impl Repo {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("debuggatha-{name}-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).unwrap();
        let repo = Self(dir);
        repo.git(&["init", "-q", "-b", "main"]);
        repo
    }

    fn git(&self, args: &[&str]) {
        let status = Command::new("git")
            .args([
                "-c",
                "user.name=t",
                "-c",
                "user.email=t@t",
                "-c",
                "commit.gpgsign=false",
            ])
            .args(args)
            .current_dir(&self.0)
            .status()
            .unwrap();
        assert!(status.success(), "git {args:?}");
    }

    fn write(&self, path: &str, content: impl AsRef<[u8]>) {
        let full = self.0.join(path);
        std::fs::create_dir_all(full.parent().unwrap()).unwrap();
        std::fs::write(full, content).unwrap();
    }
}

impl Drop for Repo {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).ok();
    }
}

fn lines_of(review: &debuggatha_engine::diff_review::DiffReview) -> Vec<(String, u32)> {
    review
        .run
        .findings
        .iter()
        .map(|f| {
            let l = &f.locations[0];
            (l.file.clone(), l.lines.unwrap().start)
        })
        .collect()
}

fn review(root: &Path, base: Option<&str>) -> debuggatha_engine::diff_review::DiffReview {
    review_diff(root, base, &policy(), &default_registry(), &FixedEnv).expect("review")
}

#[test]
fn only_what_the_change_added_is_a_finding() {
    let repo = Repo::new("added");
    repo.write("old.js", "eval(a);\nconst keep = 1;\n");
    repo.write("gone.js", "eval(z);\n");
    repo.git(&["add", "."]);
    repo.git(&["commit", "-q", "-m", "base"]);

    repo.write("old.js", "eval(a);\nconst keep = 1;\neval(b);\n");
    repo.write("fresh.js", "const x = 1;\neval(c);\n");
    repo.write("image.bin", [0_u8, 159, 146, 150, 0, 1]);
    repo.write("node_modules/dep/index.js", "eval(d);\n");
    std::fs::remove_file(repo.0.join("gone.js")).unwrap();

    let r = review(&repo.0, None);
    assert_eq!(
        lines_of(&r),
        [("fresh.js".to_owned(), 2), ("old.js".to_owned(), 3)],
        "the eval that was already in old.js is not this change's"
    );
    assert_eq!(r.deleted, ["gone.js"]);
    let reasons: Vec<_> = r.skipped.iter().map(|s| (s.reason, s.files)).collect();
    assert!(reasons.contains(&(SkipReason::NotUtf8, 1)), "{reasons:?}");
    assert_eq!(r.base.as_deref(), Some("HEAD"));
}

#[test]
fn a_base_ref_measures_against_it() {
    let repo = Repo::new("base");
    repo.write("a.js", "const a = 1;\n");
    repo.git(&["add", "."]);
    repo.git(&["commit", "-q", "-m", "one"]);
    repo.write("a.js", "const a = 1;\neval(x);\n");
    repo.git(&["commit", "-q", "-am", "two"]);

    assert_eq!(lines_of(&review(&repo.0, None)), []);
    assert_eq!(
        lines_of(&review(&repo.0, Some("HEAD~1"))),
        [("a.js".to_owned(), 2)]
    );
}

#[test]
fn a_repository_with_no_commit_reviews_every_file() {
    let repo = Repo::new("nocommit");
    repo.write("a.js", "eval(x);\n");
    let r = review(&repo.0, None);
    assert_eq!(lines_of(&r), [("a.js".to_owned(), 1)]);
    assert_eq!(r.base, None);
}

#[test]
fn an_unsafe_ref_never_reaches_git_and_a_plain_directory_is_not_a_repository() {
    let repo = Repo::new("refs");
    let err = review_diff(
        &repo.0,
        Some("--output=/tmp/x"),
        &policy(),
        &default_registry(),
        &FixedEnv,
    )
    .unwrap_err();
    assert!(matches!(err, DiffReviewError::Git(GitError::UnsafeRef)));
    assert!(!repo.0.join("..").join("x").exists());

    let plain = std::env::temp_dir().join(format!("debuggatha-plain-{}", std::process::id()));
    std::fs::create_dir_all(&plain).unwrap();
    let err = review_diff(&plain, None, &policy(), &default_registry(), &FixedEnv).unwrap_err();
    std::fs::remove_dir_all(&plain).ok();
    assert!(matches!(
        err,
        DiffReviewError::Git(GitError::NotARepository)
    ));
    assert!(
        !err.to_string().contains("debuggatha-plain"),
        "no raw path in the message"
    );
}
