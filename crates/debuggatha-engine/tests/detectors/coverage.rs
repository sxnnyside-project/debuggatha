//! The coverage report says what a review could not check. Each case builds a run from real
//! parses (nothing mocked) and compares the whole report; the serialized form must satisfy the
//! schema in `docs/schemas`.
#![allow(clippy::expect_used, clippy::unwrap_used)] // a failed expect is a failed test

use debuggatha_core::env::Env;
use debuggatha_core::knowledge_system::ReviewPolicy;
use debuggatha_engine::detect::coverage::{Coverage, SkipReason, SkippedFiles};
use debuggatha_engine::detect::{SourceUnit, default_registry, run_review};
use serde_json::{Value, json};

struct FixedEnv;
impl Env for FixedEnv {
    fn now(&self) -> String {
        "2026-01-01T00:00:00.000Z".into()
    }
    fn new_id(&self) -> String {
        "id".into()
    }
}

fn policy(rules: &[(&str, Option<(&str, &str)>)]) -> ReviewPolicy {
    let rules: Vec<Value> = rules
        .iter()
        .map(|(id, pack)| {
            let origin = pack.map_or_else(
                || json!({ "kind": "criteria" }),
                |(p, v)| json!({ "kind": "pack", "packId": p, "packVersion": v }),
            );
            json!({ "id": id, "statement": "s.", "origin": origin })
        })
        .collect();
    serde_json::from_value(json!({
        "id": "p", "rules": rules, "packRefs": [], "conflicts": []
    }))
    .expect("policy")
}

fn unit(file: &str, content: &str) -> SourceUnit {
    SourceUnit {
        file: file.into(),
        content: content.into(),
        changed_lines: None,
    }
}

fn coverage(
    policy: &ReviewPolicy,
    units: &[SourceUnit],
    skipped: Vec<SkippedFiles>,
    detail: bool,
) -> Coverage {
    let registry = default_registry();
    let run = run_review(policy, units, &registry, &FixedEnv).expect("run");
    run.coverage(policy, &registry, skipped, detail)
}

fn valid(c: &Coverage) {
    let schema: Value = serde_json::from_str(
        &std::fs::read_to_string(format!(
            "{}/../../docs/schemas/review-result.schema.json",
            env!("CARGO_MANIFEST_DIR")
        ))
        .expect("schema"),
    )
    .expect("JSON");
    let wrapper = json!({
        "$ref": "#/definitions/Coverage",
        "definitions": schema["definitions"],
    });
    let validator = jsonschema::draft7::new(&wrapper).expect("compiles");
    let value = serde_json::to_value(c).expect("json");
    assert!(validator.is_valid(&value), "{value}");
}

#[test]
fn a_mixed_repository_reports_each_limit() {
    let p = policy(&[
        ("no-var", Some(("javascript", "1.0.0"))),
        ("no-explicit-any", Some(("typescript", "2.0.0"))),
        ("made-up-rule", Some(("javascript", "1.0.0"))),
        ("our-own-criterion", None),
    ]);
    let units = [
        unit("a.ts", "export const a = 1;\n"),
        unit("b.ts", "const a;\nconst ok = 1;\n"),
        unit("c.py", "x = 1\n"),
        unit("d.scala", "object A\n"),
        unit("e.cpp", "int main() { return 0; }\n"),
        unit("Makefile", "all:\n"),
    ];
    let c = coverage(&p, &units, vec![], false);
    valid(&c);
    assert_eq!(
        serde_json::to_value(&c).unwrap(),
        json!({
            "complete": false,
            "languages": [
                { "language": "cpp", "files": 1, "filesWithErrors": 0,
                  "status": "parsed-limited", "parser": "tree-sitter",
                  "reason": "the grammar does not run the preprocessor, so macro-heavy code reads as errors" },
                { "language": "python", "files": 1, "filesWithErrors": 0,
                  "status": "parsed", "parser": "tree-sitter" },
                { "language": "scala", "files": 1, "filesWithErrors": 0,
                  "status": "not-parsed", "parser": "none",
                  "reason": "no parser for this language: only text-level rules ran" },
                { "language": "typescript", "files": 2, "filesWithErrors": 1,
                  "status": "parsed-with-errors", "parser": "oxc",
                  "reason": "1 file(s) with syntax errors" },
                { "language": "unknown", "files": 1, "filesWithErrors": 0,
                  "status": "not-parsed", "parser": "none",
                  "reason": "no parser for this language: only text-level rules ran" },
            ],
            "checkedRules": [
                { "packId": "javascript", "packVersion": "1.0.0", "ruleIds": ["no-var"] },
                { "packId": "typescript", "packVersion": "2.0.0", "ruleIds": ["no-explicit-any"] },
            ],
            "ruleGaps": [
                { "packId": "javascript", "ruleId": "made-up-rule", "reason": "no-detector" },
                { "packId": "criteria", "ruleId": "our-own-criterion", "reason": "no-detector" },
            ],
            "skippedFiles": [],
        })
    );
}

#[test]
fn file_detail_only_when_asked() {
    let p = policy(&[("no-var", Some(("javascript", "1.0.0")))]);
    let units = [
        unit("a.ts", "export const a = 1;\n"),
        unit("b.ts", "const a;\n"),
    ];
    let summary = coverage(&p, &units, vec![], false);
    assert!(summary.languages[0].file_details.is_none());
    let detailed = coverage(&p, &units, vec![], true);
    valid(&detailed);
    let files = detailed.languages[0].file_details.as_ref().unwrap();
    assert_eq!(
        files
            .iter()
            .map(|f| (f.path.as_str(), serde_json::to_value(f.status).unwrap()))
            .collect::<Vec<_>>(),
        [
            ("a.ts", json!("parsed")),
            ("b.ts", json!("parsed-with-errors"))
        ]
    );
}

#[test]
fn complete_needs_every_condition() {
    let p = policy(&[("no-var", Some(("javascript", "1.0.0")))]);
    let ok = [unit("a.ts", "export const a = 1;\n")];
    assert!(coverage(&p, &ok, vec![], false).complete);

    // A scope decision does not hide code the person asked about.
    let ignored = vec![SkippedFiles {
        reason: SkipReason::Ignored,
        files: 3,
        examples: (0..9).map(|i| format!("f{i}")).collect(),
    }];
    let c = coverage(&p, &ok, ignored, false);
    assert!(c.complete);
    assert_eq!(c.skipped_files[0].examples.len(), 5);
    valid(&c);

    for reason in [
        SkipReason::TooLarge,
        SkipReason::Unreadable,
        SkipReason::NotUtf8,
    ] {
        let hidden = vec![SkippedFiles {
            reason,
            files: 1,
            examples: vec![],
        }];
        assert!(!coverage(&p, &ok, hidden, false).complete, "{reason:?}");
    }
    // Zero files is not a skip.
    let none = vec![SkippedFiles {
        reason: SkipReason::TooLarge,
        files: 0,
        examples: vec![],
    }];
    assert!(coverage(&p, &ok, none, false).complete);

    let gap = policy(&[
        ("no-var", Some(("javascript", "1.0.0"))),
        ("made-up-rule", Some(("javascript", "1.0.0"))),
    ]);
    assert!(!coverage(&gap, &ok, vec![], false).complete);
    let scala = [
        unit("a.ts", "export const a = 1;\n"),
        unit("b.scala", "object B\n"),
    ];
    assert!(!coverage(&p, &scala, vec![], false).complete);
    let broken = [unit("a.ts", "const a;\n")];
    assert!(!coverage(&p, &broken, vec![], false).complete);
}

#[test]
fn a_file_the_parser_cannot_read_is_not_parsed_and_counts_as_an_error() {
    let p = policy(&[("no-var", Some(("javascript", "1.0.0")))]);
    let units = [unit("a.ts", "export const a = 1;\n"), unit("b.ts", "}}}}")];
    let c = coverage(&p, &units, vec![], true);
    valid(&c);
    let l = &c.languages[0];
    assert_eq!(l.files_with_errors, 1);
    assert_eq!(
        l.reason.as_deref(),
        Some("1 file(s) with syntax errors, 1 not readable at all")
    );
    let details = l.file_details.as_ref().unwrap();
    assert_eq!(
        serde_json::to_value(details[1].status).unwrap(),
        json!("not-parsed")
    );
    assert!(!c.complete);
}

#[test]
fn a_rule_listed_twice_is_checked_once() {
    let p = policy(&[
        ("no-var", Some(("javascript", "1.0.0"))),
        ("no-var", Some(("javascript", "1.0.0"))),
    ]);
    let c = coverage(&p, &[], vec![], false);
    assert_eq!(c.checked_rules[0].rule_ids, ["no-var"]);
    assert!(c.languages.is_empty() && c.complete);
}

#[test]
fn a_rule_checked_in_part_is_also_a_gap_that_needs_types() {
    let p = policy(&[
        ("no-var", Some(("javascript", "1.0.0"))),
        ("no-floating-promises", Some(("typescript", "2.0.0"))),
    ]);
    let ok = [unit("a.ts", "export const a = 1;\n")];
    let c = coverage(&p, &ok, vec![], false);
    valid(&c);
    // It ran, so it is among the checked rules...
    assert!(
        c.checked_rules
            .iter()
            .any(|r| r.pack_id == "typescript" && r.rule_ids == ["no-floating-promises"])
    );
    // ...and the part it cannot check is stated, so the review is not complete.
    let gaps = serde_json::to_value(&c.rule_gaps).unwrap();
    assert_eq!(gaps.as_array().unwrap().len(), 1);
    assert_eq!(gaps[0]["ruleId"], "no-floating-promises");
    assert_eq!(gaps[0]["packId"], "typescript");
    assert_eq!(gaps[0]["reason"], "needs-type-information");
    assert!(gaps[0]["detail"].as_str().unwrap().contains("needs types"));
    assert!(!c.complete);
}

#[test]
fn a_partial_rule_a_policy_does_not_hold_adds_no_gap() {
    let p = policy(&[("no-var", Some(("javascript", "1.0.0")))]);
    let ok = [unit("a.ts", "export const a = 1;\n")];
    let c = coverage(&p, &ok, vec![], false);
    assert!(c.rule_gaps.is_empty() && c.complete);
}
