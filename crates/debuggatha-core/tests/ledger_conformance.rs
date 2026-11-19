//! Differential tests against the TypeScript ledger (ADR-0006, decision 8).
//!
//! `tests/fixtures/ledger-conformance.json` is written by `scripts/parity/gen-ledger-fixtures.ts` at tag `ts-engine-final`, which
//! runs the TypeScript implementation. Ids are renumbered `n1`, `n2`, ... in creation order and
//! timestamps are not compared, so the Rust side injects a counting environment.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)] // a failed expect is a failed test

mod common;

use common::{CountingEnv, fixture};
use debuggatha_core::findings_ledger::{
    Ledger, LedgerEntry, LifecycleStatus, ReviewAssociation, SyncScope, TransitionOrigin,
    compute_finding_fingerprint, create_empty_ledger, deserialize_ledger, load_ledger, save_ledger,
    serialize_ledger, snapshot_ref_for, summarize_ledger, synchronize_review_result,
    update_finding_status,
};
use debuggatha_core::review_engine::{
    ExecutionMetadata, Finding, ReviewResult, Severity, summarize_findings,
};
use serde_json::{Value, json};

fn project(ledger: &Ledger) -> Value {
    Value::Array(ledger.entries.iter().map(project_entry).collect())
}

fn project_entry(e: &LedgerEntry) -> Value {
    let history: Vec<Value> = e
        .history
        .iter()
        .map(|h| {
            let mut v = json!({ "newState": h.new_state, "origin": h.origin });
            if let Some(p) = h.previous_state {
                v["previousState"] = json!(p);
            }
            if let Some(c) = &h.comment {
                v["comment"] = json!(c);
            }
            v
        })
        .collect();
    json!({
        "id": e.id,
        "status": e.status,
        "fingerprint": e.fingerprint,
        "latestFindingId": e.latest_finding.id,
        "review": {
            "createdBySessionId": e.review.created_by_session_id,
            "lastUpdatedBySessionId": e.review.last_updated_by_session_id,
        },
        "history": history,
    })
}

fn scope_of(value: &Value) -> SyncScope {
    if value["kind"] == "workspace" {
        SyncScope::Workspace
    } else {
        SyncScope::Files(
            value["files"]
                .as_array()
                .unwrap()
                .iter()
                .map(|f| f.as_str().unwrap().to_owned())
                .collect(),
        )
    }
}

fn status_of(value: &Value) -> LifecycleStatus {
    serde_json::from_value(value.clone()).expect("a lifecycle status")
}

#[test]
fn scenarios_match_the_typescript_ledger() {
    let doc = fixture("ledger-conformance.json");
    let env = CountingEnv::new("n");
    for scenario in doc["scenarios"].as_array().unwrap() {
        let name = scenario["name"].as_str().unwrap();
        let mut ledger = create_empty_ledger("/repo", &env);
        for (i, step) in scenario["steps"].as_array().unwrap().iter().enumerate() {
            let action = &step["action"];
            let expected = &step["expected"];
            let at = format!("{name}, step {}", i + 1);
            if action["type"] == "sync" {
                let findings: Vec<Finding> =
                    serde_json::from_value(action["findings"].clone()).unwrap();
                let association: ReviewAssociation =
                    serde_json::from_value(action["association"].clone()).unwrap();
                let result = ReviewResult {
                    session_id: action["sessionId"].as_str().unwrap().to_owned(),
                    summary: summarize_findings(&findings, None, vec![], vec![]),
                    findings,
                    recommendations: vec![],
                    execution: ExecutionMetadata::default(),
                };
                let (next, report) = synchronize_review_result(
                    &ledger,
                    &result,
                    &association,
                    &scope_of(&action["scope"]),
                    &env,
                )
                .expect("sync");
                ledger = next;
                assert_eq!(
                    serde_json::to_value(&report).unwrap(),
                    expected["report"],
                    "{at}: report"
                );
            } else {
                let id = action["entryId"].as_str().unwrap();
                let origin: TransitionOrigin =
                    serde_json::from_value(action["origin"].clone()).unwrap();
                let comment = action["comment"].as_str().map(str::to_owned);
                match update_finding_status(
                    &ledger,
                    id,
                    status_of(&action["to"]),
                    origin,
                    comment,
                    &env,
                ) {
                    Ok(next) => ledger = next,
                    Err(error) => {
                        assert_eq!(json!(error.to_string()), expected["error"], "{at}: error");
                        continue;
                    }
                }
                assert!(
                    expected.get("error").is_none(),
                    "{at}: TypeScript refused, Rust accepted"
                );
            }
            assert_eq!(project(&ledger), expected["entries"], "{at}: entries");
            assert_eq!(
                serde_json::to_value(summarize_ledger(&ledger)).unwrap(),
                expected["summary"],
                "{at}: summary"
            );
        }
    }
}

#[test]
fn fingerprints_are_byte_identical_for_non_ascii_excerpts() {
    let doc = fixture("ledger-conformance.json");
    let vectors = doc["fingerprints"].as_array().unwrap();
    assert!(vectors.len() >= 10);
    for v in vectors {
        let excerpt = v["excerpt"].as_str().unwrap();
        let finding: Finding = serde_json::from_value(json!({
            "id": "fp", "title": "fp", "explanation": "x", "severity": "medium", "confidence": "high",
            "category": "security",
            "locations": [{ "file": "src/fp.ts", "lines": { "start": 1, "end": 1 } }],
            "evidence": [{ "kind": "code", "file": "src/fp.ts", "excerpt": excerpt }],
            "recommendations": []
        }))
        .unwrap();
        assert_eq!(
            serde_json::to_value(compute_finding_fingerprint(&finding)).unwrap(),
            v["fingerprint"],
            "excerpt {excerpt:?}"
        );
    }
}

#[test]
fn real_findings_survive_a_round_trip_unchanged() {
    let doc = fixture("real-findings.json");
    let findings = doc["findings"].as_array().unwrap();
    assert_eq!(findings.len(), 80);
    for original in findings {
        let finding: Finding =
            serde_json::from_value(original.clone()).expect("a real finding parses");
        assert_eq!(
            &serde_json::to_value(&finding).unwrap(),
            original,
            "finding {}",
            finding.id
        );
    }
    let severities: std::collections::BTreeSet<Severity> = findings
        .iter()
        .map(|f| serde_json::from_value(f["severity"].clone()).unwrap())
        .collect();
    assert!(
        severities.len() > 1,
        "the fixture should cover several severities"
    );
}

#[test]
fn canonical_json_matches_javascript() {
    let doc = fixture("ledger-conformance.json");
    for case in doc["canonical"].as_array().unwrap() {
        let input: Value = serde_json::from_str(case["json"].as_str().unwrap()).unwrap();
        assert_eq!(
            debuggatha_core::canonical::canonical_stringify(&input),
            case["expected"].as_str().unwrap(),
            "input {}",
            case["json"]
        );
    }
}

#[test]
fn snapshot_hashes_of_real_repository_contexts_match() {
    let doc = fixture("ledger-conformance.json");
    for case in doc["snapshots"].as_array().unwrap() {
        let p = &case["parts"];
        let snapshot = snapshot_ref_for(
            "t",
            &p["stack"],
            &p["dependencies"],
            &p["documentation"],
            &p["criteria"],
        );
        assert_eq!(
            snapshot.fingerprint,
            case["expected"].as_str().unwrap(),
            "{}",
            case["dir"]
        );
    }
}

#[test]
fn serialized_ledger_is_byte_identical_and_reads_back() {
    let doc = fixture("ledger-conformance.json");
    let env = CountingEnv::new("n");
    for scenario in doc["scenarios"].as_array().unwrap() {
        let mut ledger = create_empty_ledger("/repo", &env);
        for step in scenario["steps"].as_array().unwrap() {
            let action = &step["action"];
            if action["type"] == "sync" {
                let findings: Vec<Finding> =
                    serde_json::from_value(action["findings"].clone()).unwrap();
                let association: ReviewAssociation =
                    serde_json::from_value(action["association"].clone()).unwrap();
                let result = ReviewResult {
                    session_id: action["sessionId"].as_str().unwrap().to_owned(),
                    summary: summarize_findings(&findings, None, vec![], vec![]),
                    findings,
                    recommendations: vec![],
                    execution: ExecutionMetadata::default(),
                };
                ledger = synchronize_review_result(
                    &ledger,
                    &result,
                    &association,
                    &scope_of(&action["scope"]),
                    &env,
                )
                .unwrap()
                .0;
            } else if let Ok(next) = update_finding_status(
                &ledger,
                action["entryId"].as_str().unwrap(),
                status_of(&action["to"]),
                serde_json::from_value(action["origin"].clone()).unwrap(),
                action["comment"].as_str().map(str::to_owned),
                &env,
            ) {
                ledger = next;
            }
        }
        let text = serialize_ledger(&ledger).unwrap();
        assert_eq!(
            text,
            scenario["serialized"].as_str().unwrap(),
            "{}",
            scenario["name"]
        );
        assert_eq!(
            deserialize_ledger(&text).unwrap(),
            ledger,
            "{}: read back",
            scenario["name"]
        );
    }
}

#[test]
fn the_reader_refuses_what_the_typescript_reader_refuses() {
    let doc = fixture("ledger-conformance.json");
    for case in doc["refusals"].as_array().unwrap() {
        let error = deserialize_ledger(case["json"].as_str().unwrap()).unwrap_err();
        assert!(
            error
                .to_string()
                .contains(case["message"].as_str().unwrap()),
            "{error}"
        );
    }
}

#[test]
fn the_reader_is_stricter_than_typescript_about_shape() {
    // Intended divergence (docs/PARITY.md): unknown fields are refused, not silently dropped.
    let env = CountingEnv::new("n");
    let mut value = serde_json::to_value(create_empty_ledger("/repo", &env)).unwrap();
    value["surprise"] = json!(1);
    let error = deserialize_ledger(&value.to_string()).unwrap_err();
    assert!(error.to_string().contains("does not understand"), "{error}");
    assert!(deserialize_ledger("not json").is_err());
    assert!(deserialize_ledger("null").is_err());
}

#[test]
fn save_is_atomic_and_load_round_trips() {
    let env = CountingEnv::new("n");
    let dir = std::env::temp_dir().join(format!("debuggatha-ledger-{}", std::process::id()));
    drop(std::fs::remove_dir_all(&dir));
    std::fs::create_dir_all(&dir).unwrap();
    // A repository with no ledger has an empty one.
    let empty = load_ledger(&dir, &env).unwrap();
    assert_eq!(empty.entries, []);
    let mut ledger = create_empty_ledger(&dir.to_string_lossy(), &env);
    ledger.updated_at = "later".to_owned();
    save_ledger(&ledger).unwrap();
    assert_eq!(load_ledger(&dir, &env).unwrap(), ledger);
    let leftovers: Vec<_> = std::fs::read_dir(dir.join(".debuggatha"))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(
        leftovers,
        ["ledger.json"],
        "no temporary file is left behind"
    );
    std::fs::remove_dir_all(&dir).unwrap();
}
