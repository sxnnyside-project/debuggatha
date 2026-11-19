//! The baseline accepts what a repository has today so a review reports only what is new.
#![allow(clippy::expect_used, clippy::unwrap_used)] // a failed expect is a failed test

use debuggatha_core::env::Env;
use debuggatha_core::knowledge_system::ReviewPolicy;
use debuggatha_core::review_engine::Finding;
use debuggatha_engine::baseline::{
    apply_baseline, baseline_path, clear_baseline, create_baseline_from, load_baseline,
    save_baseline,
};
use debuggatha_engine::detect::{SourceUnit, default_registry, run_review};
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

fn findings(files: &[(&str, &str)]) -> Vec<Finding> {
    let policy: ReviewPolicy = serde_json::from_value(json!({
        "id": "p",
        "rules": [{
            "id": "no-eval",
            "statement": "Do not evaluate strings as code.",
            "origin": { "kind": "pack", "packId": "debuggatha/owasp", "packVersion": "1.0.0" }
        }],
        "packRefs": [], "conflicts": []
    }))
    .unwrap();
    let units: Vec<SourceUnit> = files
        .iter()
        .map(|(file, content)| SourceUnit {
            file: (*file).to_owned(),
            content: (*content).to_owned(),
            changed_lines: None,
        })
        .collect();
    run_review(&policy, &units, &default_registry(), &FixedEnv)
        .unwrap()
        .findings
}

fn temp(name: &str) -> std::path::PathBuf {
    let dir =
        std::env::temp_dir().join(format!("debuggatha-baseline-{name}-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn what_is_accepted_stays_quiet_and_what_is_new_is_reported() {
    let before = findings(&[("a.js", "eval(x);\n")]);
    let baseline = create_baseline_from(&before, &FixedEnv);
    assert_eq!(baseline.entries.len(), 1);

    let after = findings(&[
        ("a.js", "// moved down\n\neval(x);\neval(y);\n"),
        ("b.js", "eval(z);\n"),
    ]);
    let applied = apply_baseline(&after, &baseline);
    assert_eq!(
        applied.baselined.len(),
        1,
        "the same code on another line is still accepted"
    );
    assert_eq!(
        applied.fresh.len(),
        2,
        "a second eval and a new file are new"
    );
}

#[test]
fn identical_findings_are_matched_one_for_one() {
    let two = findings(&[("a.js", "eval(x);\neval(x);\n")]);
    let baseline = create_baseline_from(&two, &FixedEnv);
    assert_eq!(baseline.entries.len(), 1);
    assert_eq!(baseline.entries[0].count, 2);

    let three = findings(&[("a.js", "eval(x);\neval(x);\neval(x);\n")]);
    let applied = apply_baseline(&three, &baseline);
    assert_eq!((applied.baselined.len(), applied.fresh.len()), (2, 1));
}

#[test]
fn it_is_saved_loaded_and_cleared() {
    let root = temp("io");
    assert_eq!(load_baseline(&root).unwrap(), None);
    assert!(!clear_baseline(&root).unwrap());

    let baseline = create_baseline_from(&findings(&[("a.js", "eval(x);\n")]), &FixedEnv);
    save_baseline(&root, &baseline).unwrap();
    assert_eq!(load_baseline(&root).unwrap(), Some(baseline));
    assert!(clear_baseline(&root).unwrap());
    assert!(!baseline_path(&root).exists());
    std::fs::remove_dir_all(root).ok();
}

#[test]
fn a_file_written_by_the_typescript_engine_is_read() {
    let root = temp("ts");
    std::fs::create_dir_all(root.join(".debuggatha")).unwrap();
    std::fs::write(
        baseline_path(&root),
        r#"{
  "schemaVersion": 1,
  "createdAt": "2026-01-01T00:00:00.000Z",
  "entries": [
    { "key": "a.js::no-eval::abc", "file": "a.js", "ruleId": "no-eval", "title": "t", "count": 2 }
  ]
}
"#,
    )
    .unwrap();
    let baseline = load_baseline(&root).unwrap().unwrap();
    assert_eq!(baseline.entries[0].count, 2);
    std::fs::remove_dir_all(root).ok();
}

#[test]
fn a_damaged_or_foreign_file_is_refused_not_overwritten() {
    let root = temp("bad");
    std::fs::create_dir_all(root.join(".debuggatha")).unwrap();
    for text in [
        "not json",
        r#"{"schemaVersion":2,"createdAt":"x","entries":[]}"#,
        r#"{"schemaVersion":1,"createdAt":"x","entries":[],"extra":1}"#,
    ] {
        std::fs::write(baseline_path(&root), text).unwrap();
        let error = load_baseline(&root).unwrap_err().to_string();
        assert!(error.starts_with("Cannot read baseline"), "{error}");
        assert!(
            !error.contains("debuggatha-baseline"),
            "no raw path: {error}"
        );
    }
    std::fs::remove_dir_all(root).ok();
}
