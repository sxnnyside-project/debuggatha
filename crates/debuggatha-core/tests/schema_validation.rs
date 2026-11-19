//! The generated schemas accept what the TypeScript engine produced (the 61 golden contexts and
//! the session fixtures) and reject what they must not.
#![allow(clippy::expect_used, clippy::unwrap_used)] // a failed expect is a failed test

mod common;

use serde_json::{Value, json};

fn schema(name: &str) -> jsonschema::Validator {
    let path = format!(
        "{}/../../docs/schemas/{name}.schema.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let value: Value =
        serde_json::from_str(&std::fs::read_to_string(path).expect("schema")).expect("JSON");
    jsonschema::draft7::new(&value).expect("compiles")
}

fn golden_contexts() -> Vec<(String, Value)> {
    let dir = format!(
        "{}/../../corpus/golden/ts-context",
        env!("CARGO_MANIFEST_DIR")
    );
    let mut out: Vec<_> = std::fs::read_dir(dir)
        .expect("golden dir")
        .map(|e| e.expect("entry").path())
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .map(|p| {
            let v: Value =
                serde_json::from_str(&std::fs::read_to_string(&p).expect("read")).expect("JSON");
            (
                p.file_stem().unwrap().to_string_lossy().into_owned(),
                v["context"].clone(),
            )
        })
        .collect();
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

#[test]
fn all_golden_contexts_are_valid() {
    let v = schema("repository-context");
    let contexts = golden_contexts();
    assert!(
        contexts.len() >= 61,
        "golden corpus missing: {}",
        contexts.len()
    );
    for (name, ctx) in &contexts {
        let errors: Vec<_> = v.iter_errors(ctx).map(|e| e.to_string()).collect();
        assert!(errors.is_empty(), "{name}: {errors:?}");
    }
}

#[test]
fn contexts_that_must_be_rejected_are() {
    let v = schema("repository-context");
    let (_, base) = golden_contexts().remove(0);
    let with = |f: &dyn Fn(&mut Value)| {
        let mut c = base.clone();
        f(&mut c);
        c
    };
    let bad = [
        ("unknown field", with(&|c| c["extra"] = json!(1))),
        (
            "missing stack",
            with(&|c| {
                c.as_object_mut().unwrap().remove("stack");
            }),
        ),
        (
            "bad workspace type",
            with(&|c| c["stack"]["workspaceType"] = json!("polyrepo")),
        ),
        (
            "bad confidence",
            with(&|c| c["understanding"]["confidence"] = json!("certain")),
        ),
        (
            "negative word count",
            with(&|c| {
                c["documentation"]["sources"] =
                    json!([{ "file": "a", "kind": "readme", "wordCount": -1 }]);
            }),
        ),
        (
            "evidence without detail",
            with(&|c| {
                c["stack"]["workspaceEvidence"] = json!([{ "file": "a" }]);
            }),
        ),
        (
            "version is a number",
            with(&|c| c["dependencies"]["declaredVersions"] = json!({ "a": 1 })),
        ),
    ];
    for (name, ctx) in bad {
        assert!(!v.is_valid(&ctx), "{name} was accepted");
    }
}

#[test]
fn session_and_request_fixtures_are_valid_and_bad_ones_are_not() {
    let mut fx = common::fixture("session-conformance.json");
    // The fixture normalizes the context's build time to "T"; the schema checks it is a date-time.
    let at = json!("2026-07-10T00:00:00.000Z");
    fx["session"]["repositoryContext"]["generatedAt"] = at.clone();
    for walk in fx["sessions"].as_array_mut().unwrap() {
        walk["final"]["repositoryContext"]["generatedAt"] = at.clone();
    }
    let session = schema("review-session");
    let request = schema("review-request");
    assert!(session.is_valid(&fx["session"]));
    for walk in fx["sessions"].as_array().unwrap() {
        assert!(session.is_valid(&walk["final"]), "{}", walk["name"]);
    }
    for case in fx["requests"].as_array().unwrap() {
        if let Some(r) = case.get("request") {
            assert!(request.is_valid(r), "{}", case["name"]);
        }
    }
    let r0 = fx["requests"][0]["request"].clone();
    let mut bad_status = fx["session"].clone();
    bad_status["status"] = json!("bogus");
    assert!(!session.is_valid(&bad_status));
    for (name, patch) in [
        ("unknown scope", json!({ "scope": { "kind": "nope" } })),
        (
            "empty files",
            json!({ "scope": { "kind": "files", "paths": [] } }),
        ),
        ("bad depth", json!({ "depth": "deep" })),
    ] {
        let mut r = r0.clone();
        for (k, v) in patch.as_object().unwrap() {
            r[k] = v.clone();
        }
        assert!(!request.is_valid(&r), "{name} was accepted");
    }
}

#[test]
fn a_malformed_timestamp_is_rejected() {
    let fx = common::fixture("session-conformance.json");
    assert!(
        !schema("review-session").is_valid(&fx["session"]),
        "\"T\" is not a date-time"
    );
}

fn golden_findings() -> Vec<(String, Vec<Value>)> {
    use std::io::Read;
    let dir = format!(
        "{}/../../corpus/golden/ts-findings",
        env!("CARGO_MANIFEST_DIR")
    );
    let mut out = Vec::new();
    for entry in std::fs::read_dir(dir).expect("dir") {
        let path = entry.expect("entry").path();
        let mut text = String::new();
        flate2::read::GzDecoder::new(std::fs::File::open(&path).expect("open"))
            .read_to_string(&mut text)
            .expect("gunzip");
        let findings: Vec<Value> = serde_json::from_str(&text).expect("JSON");
        out.push((
            path.file_name().unwrap().to_string_lossy().into_owned(),
            findings,
        ));
    }
    out
}

#[test]
fn every_golden_finding_is_valid() {
    let v = schema("finding");
    let mut n = 0;
    for (name, findings) in golden_findings() {
        for f in &findings {
            let errors: Vec<_> = v.iter_errors(f).map(|e| e.to_string()).collect();
            assert!(errors.is_empty(), "{name}: {errors:?}");
            n += 1;
        }
    }
    assert!(n >= 7_736, "golden findings missing: {n}");
}

#[test]
fn findings_that_must_be_rejected_are() {
    let v = schema("finding");
    let base = golden_findings()
        .into_iter()
        .find_map(|(_, f)| f.into_iter().next())
        .expect("a finding");
    let with = |f: &dyn Fn(&mut Value)| {
        let mut c = base.clone();
        f(&mut c);
        c
    };
    let bad = [
        ("no evidence", with(&|c| c["evidence"] = json!([]))),
        ("no locations", with(&|c| c["locations"] = json!([]))),
        (
            "severity urgent",
            with(&|c| c["severity"] = json!("urgent")),
        ),
        ("unknown field", with(&|c| c["extra"] = json!(1))),
        (
            "line zero",
            with(&|c| c["locations"][0]["lines"] = json!({ "start": 0, "end": 1 })),
        ),
        ("null policy", with(&|c| c["appliedPolicy"] = Value::Null)),
    ];
    for (name, f) in bad {
        assert!(!v.is_valid(&f), "{name} was accepted");
    }
}

/// What the Rust ledger and memory write is what the published schemas describe.
#[test]
fn stored_files_written_by_rust_are_valid() {
    let ledger = common::fixture("ledger-conformance.json");
    let memory = common::fixture("memory-conformance.json");
    let ledger_schema = schema("ledger");
    let memory_schema = schema("memory-store");
    for scenario in ledger["scenarios"].as_array().unwrap() {
        let stored: Value = serde_json::from_str(scenario["serialized"].as_str().unwrap()).unwrap();
        let errors: Vec<_> = ledger_schema
            .iter_errors(&stored)
            .map(|e| e.to_string())
            .collect();
        assert!(errors.is_empty(), "ledger: {errors:?}");
    }
    let stored: Value = serde_json::from_str(
        // The fixture normalizes every timestamp to "T"; the schema checks they are date-times.
        &memory["serialized"]
            .as_str()
            .unwrap()
            .replace("\"T\"", "\"2026-07-10T00:00:00.000Z\""),
    )
    .unwrap();
    let errors: Vec<_> = memory_schema
        .iter_errors(&stored)
        .map(|e| e.to_string())
        .collect();
    assert!(errors.is_empty(), "memory: {errors:?}");
    let mut bad = stored;
    bad["items"][0]["status"] = json!("bogus");
    assert!(!memory_schema.is_valid(&bad));
}
