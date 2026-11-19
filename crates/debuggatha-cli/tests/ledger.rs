//! The ledger and baseline commands, run as the binary.
#![allow(clippy::expect_used, clippy::unwrap_used)] // a failed expect is a failed test

use std::path::PathBuf;
use std::process::{Command, Output};

struct Repo(PathBuf);

impl Repo {
    fn new(name: &str) -> Self {
        let dir =
            std::env::temp_dir().join(format!("debuggatha-led-{name}-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("package.json"),
            r#"{"name":"t","version":"1.0.0","dependencies":{"express":"4.0.0"}}"#,
        )
        .unwrap();
        std::fs::write(dir.join("a.js"), "const x = eval(input);\n").unwrap();
        Self(dir)
    }

    fn cli(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_debuggatha"))
            .arg("--root")
            .arg(&self.0)
            .args(args)
            .output()
            .unwrap()
    }

    fn json(&self, args: &[&str]) -> serde_json::Value {
        let mut all = vec!["--format", "json"];
        all.extend_from_slice(args);
        serde_json::from_slice(&self.cli(&all).stdout).unwrap()
    }
}

impl Drop for Repo {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).ok();
    }
}

fn code(output: &Output) -> i32 {
    output.status.code().unwrap()
}

fn text(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

#[test]
fn a_finding_can_be_listed_shown_resolved_and_reopened() {
    let repo = Repo::new("lifecycle");
    assert_eq!(code(&repo.cli(&["review", "--all"])), 2);

    let list = repo.json(&["findings", "list"]);
    assert_eq!(list.as_array().unwrap().len(), 1);
    let id = list[0]["id"].as_str().unwrap().to_owned();

    let shown = repo.cli(&["findings", "show", &id]);
    assert!(text(&shown).contains("Status: open"), "{}", text(&shown));

    assert_eq!(code(&repo.cli(&["findings", "resolve", &id])), 0);
    assert_eq!(
        repo.json(&["findings", "list"]).as_array().unwrap().len(),
        0
    );
    assert_eq!(
        repo.json(&["findings", "summary"])["countsByStatus"]["resolved"],
        1
    );

    assert_eq!(code(&repo.cli(&["findings", "reopen", &id])), 0);
    assert!(text(&repo.cli(&["findings", "show", &id])).contains("Status: reopened"));
}

#[test]
fn an_unknown_finding_and_an_illegal_move_exit_1() {
    let repo = Repo::new("illegal");
    repo.cli(&["review", "--all"]);
    let missing = repo.cli(&["findings", "resolve", "nope"]);
    assert_eq!(code(&missing), 1);
    assert!(String::from_utf8_lossy(&missing.stderr).contains("not found"));

    let id = repo.json(&["findings", "list"])[0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let reopen_open = repo.cli(&["findings", "reopen", &id]);
    assert_eq!(code(&reopen_open), 1, "an open finding cannot be reopened");
}

#[test]
fn a_baseline_is_created_shown_and_cleared() {
    let repo = Repo::new("baseline");
    assert!(text(&repo.cli(&["baseline", "show"])).contains("No baseline"));

    let created = repo.cli(&["baseline", "create"]);
    assert_eq!(code(&created), 0, "{}", text(&created));
    assert!(text(&created).contains("Baseline recorded: 1 findings across 1 files."));
    assert!(repo.0.join(".debuggatha/baseline.json").exists());
    assert!(
        !repo.0.join(".debuggatha/ledger.json").exists(),
        "the ledger is not touched"
    );

    assert!(text(&repo.cli(&["baseline", "show"])).contains("1 accepted findings"));
    assert_eq!(
        code(&repo.cli(&["review", "--all", "--no-persist"])),
        0,
        "accepted"
    );
    assert_eq!(
        code(&repo.cli(&["review", "--all", "--no-persist", "--include-baselined"])),
        2
    );

    assert!(text(&repo.cli(&["baseline", "clear"])).contains("Baseline removed."));
    assert_eq!(code(&repo.cli(&["review", "--all", "--no-persist"])), 2);
}

mod context {
    use debuggatha_engine::core::env::SystemEnv;
    use debuggatha_engine::core::repository_memory::{
        MemoryConfidence, MemoryDetail, MemoryOrigin, NewMemoryItem, add_memory_item,
        create_empty_memory_store, create_memory_item, save_memory_store,
    };

    use super::*;

    fn seed_convention(repo: &Repo) -> String {
        let item = create_memory_item(
            NewMemoryItem {
                confidence: MemoryConfidence::Inferred,
                rationale: "seen in CONTRIBUTING".into(),
                evidence: vec![],
                detail: MemoryDetail::Convention {
                    rule: "Use tabs".into(),
                },
                origin: MemoryOrigin::Detection {
                    session_id: "s".into(),
                },
            },
            &SystemEnv,
        );
        let id = item.id.clone();
        let store = create_empty_memory_store(&repo.0.to_string_lossy(), &SystemEnv);
        save_memory_store(&add_memory_item(&store, item, &SystemEnv)).unwrap();
        id
    }

    #[test]
    fn memory_follows_its_lifecycle_and_only_a_person_confirms() {
        let repo = Repo::new("memory");
        let id = seed_convention(&repo);
        assert!(text(&repo.cli(&["memory", "list"])).contains("(detected, inferred) Use tabs"));

        let skipped = repo.cli(&["memory", "activate", &id]);
        assert_eq!(code(&skipped), 1, "a detected item cannot jump to active");

        for step in ["suggest", "confirm", "activate"] {
            assert_eq!(code(&repo.cli(&["memory", step, &id])), 0, "{step}");
        }
        let active = repo.json(&["memory", "list", "--status", "active"]);
        assert_eq!(active.as_array().unwrap().len(), 1);
        assert_eq!(active[0]["history"].as_array().unwrap().len(), 4);
        assert_eq!(active[0]["history"][2]["origin"]["kind"], "user");

        assert_eq!(code(&repo.cli(&["memory", "remove", &id])), 0);
        assert_eq!(repo.json(&["memory", "list"]).as_array().unwrap().len(), 0);
        assert_eq!(code(&repo.cli(&["memory", "remove", &id])), 1);
    }

    #[test]
    fn packs_policies_and_repository_describe_what_a_review_would_use() {
        let repo = Repo::new("describe");

        let packs = repo.json(&["packs"]);
        assert!(packs.as_array().unwrap().len() > 30);

        let policy = repo.json(&["policies"]);
        assert!(policy["id"].is_string());
        assert_ne!(policy["rules"].as_array().unwrap().len(), 0);
        let review = repo.json(&["review", "--all", "--no-persist"]);
        assert_eq!(
            review["summary"]["appliedPacks"].as_array().unwrap().len(),
            policy["packRefs"].as_array().unwrap().len(),
            "the same packs a review applies"
        );

        let context = repo.json(&["repository"]);
        assert!(context["stack"]["languages"].is_array());
        assert!(text(&repo.cli(&["repository"])).contains("Stack evidence:"));
    }
}

mod health {
    use super::*;

    #[test]
    fn doctor_passes_on_a_healthy_repository_and_fails_on_a_corrupt_ledger() {
        let repo = Repo::new("doctor");
        let ok = repo.cli(&["doctor"]);
        assert_eq!(code(&ok), 0, "{}", text(&ok));
        assert!(text(&ok).contains("ok   context"));

        std::fs::create_dir_all(repo.0.join(".debuggatha")).unwrap();
        std::fs::write(repo.0.join(".debuggatha/ledger.json"), "{ not json").unwrap();
        let bad = repo.cli(&["doctor"]);
        assert_eq!(code(&bad), 1);
        assert!(text(&bad).contains("FAIL ledger"), "{}", text(&bad));
        assert!(!text(&bad).contains(repo.0.to_str().unwrap()), "no path");
    }

    #[test]
    fn completion_scripts_name_the_commands() {
        let repo = Repo::new("completion");
        for shell in ["bash", "zsh", "fish", "powershell"] {
            let out = repo.cli(&["completion", shell]);
            assert_eq!(code(&out), 0, "{shell}");
            assert!(text(&out).contains("review"), "{shell}");
        }
        assert_eq!(code(&repo.cli(&["completion", "tcsh"])), 1);
    }

    #[test]
    fn trends_count_what_the_ledger_recorded() {
        let repo = Repo::new("trends");
        assert!(text(&repo.cli(&["trends"])).contains("The ledger is empty"));

        repo.cli(&["review", "--all"]);
        let id = repo.json(&["findings", "list"])[0]["id"]
            .as_str()
            .unwrap()
            .to_owned();
        let open = repo.json(&["trends"]);
        assert_eq!(open["trends"]["open"], 1);
        assert_eq!(open["trends"]["introduced"], 1);
        assert_eq!(open["trends"]["resolved"], 0);

        repo.cli(&["findings", "resolve", &id]);
        let closed = repo.json(&["trends", "--since", "2w"]);
        assert_eq!(closed["trends"]["open"], 0);
        assert_eq!(closed["trends"]["resolved"], 1);
        assert_eq!(closed["trends"]["weeks"].as_array().unwrap().len(), 2);
        assert!(closed["trends"]["meanDaysToResolve"].is_number());

        assert_eq!(code(&repo.cli(&["trends", "--since", "0d"])), 1);
    }
}
