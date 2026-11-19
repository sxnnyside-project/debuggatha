//! The template rule of a Vue component against the TypeScript regex
//! detectors (`scripts/parity/gen-group-d-fixtures.ts`). Cases marked "same" must give exactly what the
//! regex gave: rule, line, columns, confidence and edit. Cases that carry `rust` are the ones where
//! the tree is right and the regex wrong; each states why and what Rust must report instead.
#![allow(clippy::expect_used, clippy::unwrap_used)] // a failed expect is a failed test

use std::path::Path;

use debuggatha_engine::detect::rules::template;
use debuggatha_engine::detect::{DetectorInput, Hit};
use debuggatha_engine::syntax::parse;
use serde_json::{Value, json};

fn fixture() -> Value {
    let path = format!(
        "{}/tests/fixtures/group-d-conformance.json",
        env!("CARGO_MANIFEST_DIR")
    );
    serde_json::from_str(&std::fs::read_to_string(path).expect("fixture")).expect("JSON")
}

fn run(file: &str, source: &str) -> Vec<(&'static str, Hit)> {
    let syntax = parse(Path::new(file), source).expect("a supported language");
    let input = DetectorInput::new(file, source, Some(&syntax));
    let mut hits = template::detect(&input);
    // The fixture lists hits rule by rule in the order of `RULES`.
    hits.sort_by_key(|(rule, h)| (template::RULES.iter().position(|r| r == rule), h.line));
    hits
}

fn as_json(hits: &[(&'static str, Hit)]) -> Vec<Value> {
    hits.iter()
        .map(|(rule, h)| {
            let mut v = json!({
                "rule": rule, "line": h.line, "start": h.columns.start, "end": h.columns.end,
                "confidence": h.confidence,
            });
            if let Some(e) = &h.edit {
                v["edit"] = json!({
                    "startColumn": e.start_column, "endColumn": e.end_column,
                    "replacement": e.replacement, "safety": e.safety,
                });
            }
            v
        })
        .collect()
}

#[test]
fn where_the_regex_was_right_the_tree_agrees_exactly() {
    let fx = fixture();
    let mut n = 0;
    for case in fx["cases"].as_array().unwrap() {
        if case.get("rust").is_some() {
            continue;
        }
        let got = as_json(&run(
            case["file"].as_str().unwrap(),
            case["source"].as_str().unwrap(),
        ));
        assert_eq!(Value::Array(got), case["ts"], "{}", case["name"]);
        n += 1;
    }
    assert!(n >= 15, "{n} agreeing cases");
}

#[test]
fn where_the_regex_was_wrong_the_tree_reports_what_the_case_states() {
    let fx = fixture();
    let mut bad: Vec<String> = Vec::new();
    for case in fx["cases"].as_array().unwrap() {
        let Some(rust) = case.get("rust") else {
            continue;
        };
        assert!(
            case["why"].as_str().is_some_and(|w| !w.is_empty()),
            "{}: a reason",
            case["name"]
        );
        let got: Vec<Value> = run(
            case["file"].as_str().unwrap(),
            case["source"].as_str().unwrap(),
        )
        .iter()
        .map(|(rule, h)| json!([rule, h.line, h.columns.start]))
        .collect();
        if Value::Array(got.clone()) != *rust {
            bad.push(format!(
                "{}: rust {} expected {rust}",
                case["name"],
                Value::Array(got)
            ));
            continue;
        }
        // And it really differs: otherwise the case belongs with the agreeing ones.
        let ts: Vec<Value> = case["ts"]
            .as_array()
            .unwrap()
            .iter()
            .map(|h| json!([h["rule"], h["line"], h["start"]]))
            .collect();
        assert_ne!(
            Value::Array(ts),
            *rust,
            "{}: listed as a divergence but now identical",
            case["name"]
        );
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}
