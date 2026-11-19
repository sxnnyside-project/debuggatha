//! The ledger on the whole corpus: for each of the 61 repositories, the
//! findings the TypeScript engine reported (7,736 in all, `corpus/golden/ts-findings`) go through
//! the Rust ledger, and every fingerprint and every step must equal what the TypeScript ledger did
//! (`corpus/golden/ts-ledger`). Nothing to fetch: both are committed.
//!
//! The five steps are the ones `scripts/parity/gen-ledger-golden.ts` at tag `ts-engine-final` runs: a first review; a second with
//! every fifth finding gone and every seventh one's excerpt edited; a person dismisses the first
//! open entry; everything comes back; and nothing is left in the first file.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)] // a failed expect is a failed test

mod common;

use std::io::Read;
use std::path::PathBuf;

use common::CountingEnv;
use debuggatha_core::canonical::canonical_stringify;
use debuggatha_core::findings_ledger::{
    Ledger, LifecycleStatus, SyncScope, TransitionOrigin, association_for,
    compute_finding_fingerprint, create_empty_ledger, serialize_ledger, sha256_hex,
    snapshot_ref_for, summarize_ledger, synchronize_review_result, update_finding_status,
};
use debuggatha_core::review_engine::{
    Evidence, ExecutionMetadata, Finding, PackReference, PolicyReference, ReviewResult,
    summarize_findings,
};
use serde_json::{Value, json};

fn golden_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../corpus/golden")
}

fn read_findings(name: &str) -> Vec<Finding> {
    let file = std::fs::File::open(
        golden_dir()
            .join("ts-findings")
            .join(format!("{name}.json.gz")),
    )
    .unwrap();
    let mut text = String::new();
    flate2::read::GzDecoder::new(file)
        .read_to_string(&mut text)
        .unwrap();
    serde_json::from_str(&text).unwrap()
}

/// The edit the TypeScript script applies: every code excerpt gets a line added.
fn edit(finding: &Finding) -> Finding {
    let mut edited = finding.clone();
    for evidence in &mut edited.evidence {
        if let Evidence::Code {
            excerpt: Some(excerpt),
            ..
        } = evidence
        {
            excerpt.push_str("\n// edited é");
        }
    }
    edited
}

fn project(ledger: &Ledger) -> Value {
    Value::Array(
        ledger
            .entries
            .iter()
            .map(|e| {
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
                    "id": e.id, "status": e.status, "fingerprint": e.fingerprint,
                    "latestFindingId": e.latest_finding.id,
                    "review": {
                        "createdBySessionId": e.review.created_by_session_id,
                        "lastUpdatedBySessionId": e.review.last_updated_by_session_id,
                    },
                    "history": history,
                })
            })
            .collect(),
    )
}

fn digest(value: &Value) -> String {
    sha256_hex(&canonical_stringify(value))
}

fn result_of(session: &str, findings: Vec<Finding>) -> ReviewResult {
    ReviewResult {
        session_id: session.to_owned(),
        summary: summarize_findings(
            &findings,
            None,
            vec![PackReference {
                id: "pack-a".into(),
                version: "1.0.0".into(),
            }],
            vec![PolicyReference {
                id: "policy-1".into(),
            }],
        ),
        findings,
        recommendations: vec![],
        execution: ExecutionMetadata::default(),
    }
}

struct Replay {
    env: CountingEnv,
    ledger: Ledger,
    steps: Vec<Value>,
}

impl Replay {
    fn sync(&mut self, label: &str, session: &str, findings: Vec<Finding>, scope: &SyncScope) {
        let result = result_of(session, findings);
        let snapshot = snapshot_ref_for(
            "2026-01-01T00:00:00.000Z",
            &json!([]),
            &json!([]),
            &json!([]),
            &json!([]),
        );
        let association = association_for(&result, snapshot);
        let (next, report) =
            synchronize_review_result(&self.ledger, &result, &association, scope, &self.env)
                .unwrap();
        self.ledger = next;
        let report_json = serde_json::to_value(&report).unwrap();
        let counts: serde_json::Map<String, Value> = report_json
            .as_object()
            .unwrap()
            .iter()
            .map(|(k, v)| (k.clone(), json!(v.as_array().unwrap().len())))
            .collect();
        self.steps.push(json!({
            "step": label,
            "report": counts,
            "entries": self.ledger.entries.len(),
            "digest": digest(&json!({
                "entries": project(&self.ledger),
                "report": report_json,
                "summary": summarize_ledger(&self.ledger),
            })),
        }));
    }
}

fn replay(findings: &[Finding]) -> (Replay, String) {
    let env = CountingEnv::new("n");
    let ledger = create_empty_ledger("/repo", &env);
    let mut r = Replay {
        env,
        ledger,
        steps: Vec::new(),
    };

    r.sync(
        "first review",
        "s1",
        findings.to_vec(),
        &SyncScope::Workspace,
    );
    let second: Vec<Finding> = findings
        .iter()
        .enumerate()
        .filter(|(i, _)| i % 5 != 0)
        .map(|(i, f)| if i % 7 == 3 { edit(f) } else { f.clone() })
        .collect();
    r.sync(
        "fifth removed, seventh edited",
        "s2",
        second,
        &SyncScope::Workspace,
    );

    if let Some(open) = r
        .ledger
        .entries
        .iter()
        .find(|e| e.status == LifecycleStatus::Open)
    {
        let id = open.id.clone();
        r.ledger = update_finding_status(
            &r.ledger,
            &id,
            LifecycleStatus::Dismissed,
            TransitionOrigin::Manual {
                actor: "person".into(),
            },
            Some("wontfix é".into()),
            &r.env,
        )
        .unwrap();
        r.steps.push(json!({
            "step": "dismiss first open",
            "entries": r.ledger.entries.len(),
            "digest": digest(&json!({ "entries": project(&r.ledger), "summary": summarize_ledger(&r.ledger) })),
        }));
    } else {
        r.steps
            .push(json!({ "step": "dismiss first open", "skipped": true }));
    }
    r.sync(
        "everything back",
        "s3",
        findings.to_vec(),
        &SyncScope::Workspace,
    );
    if let Some(first) = findings.first() {
        let file = first.locations[0].file.clone();
        r.sync(
            "nothing left in the first file",
            "s4",
            vec![],
            &SyncScope::Files(vec![file]),
        );
    } else {
        r.steps
            .push(json!({ "step": "nothing left in the first file", "skipped": true }));
    }
    let final_digest = digest(&Value::String(serialize_ledger(&r.ledger).unwrap()));
    (r, final_digest)
}

#[test]
fn the_ledger_gives_the_same_result_on_every_repository_of_the_corpus() {
    let mut files: Vec<_> = std::fs::read_dir(golden_dir().join("ts-ledger"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    files.sort();
    assert!(files.len() >= 60, "the ledger goldens are missing");
    let (mut findings_total, mut failures) = (0, Vec::new());
    for file in files {
        let golden: Value = serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
        let name = golden["name"].as_str().unwrap();
        let findings = read_findings(name);
        assert_eq!(
            findings.len() as u64,
            golden["findings"].as_u64().unwrap(),
            "{name}: finding count"
        );
        findings_total += findings.len();

        let fingerprints: Vec<Value> = findings
            .iter()
            .map(|f| serde_json::to_value(compute_finding_fingerprint(f)).unwrap())
            .collect();
        if Value::Array(fingerprints) != golden["fingerprints"] {
            failures.push(format!("{name}: fingerprints differ"));
            continue;
        }
        let (replayed, final_digest) = replay(&findings);
        for (got, want) in replayed
            .steps
            .iter()
            .zip(golden["steps"].as_array().unwrap())
        {
            if got != want {
                failures.push(format!(
                    "{name}: step {:?} differs\n  rust {got}\n  ts   {want}",
                    want["step"]
                ));
            }
        }
        if final_digest != golden["finalLedgerDigest"] {
            failures.push(format!("{name}: the serialized ledger differs"));
        }
    }
    assert!(
        failures.is_empty(),
        "{} problems:\n{}",
        failures.len(),
        failures.join("\n")
    );
    assert!(
        findings_total > 7000,
        "only {findings_total} findings were checked"
    );
}
