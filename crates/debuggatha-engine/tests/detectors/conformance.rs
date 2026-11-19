//! The detector runner and the finding excerpt against the TypeScript engine
//! (`scripts/parity/gen-detect-fixtures.ts` at tag `ts-engine-final`), plus what is new in the Rust runner: a detector reads the
//! whole file and a diff review keeps the hits on its added lines.
// A failed expect is a failed test; emptiness is the property several tests assert.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::assert_is_empty)]

use std::cell::Cell;
use std::collections::BTreeSet;

use debuggatha_core::env::Env;
use debuggatha_core::knowledge_system::ReviewPolicy;
use debuggatha_core::review_engine::{Confidence, EditSafety, LineRange, Severity};
use debuggatha_engine::detect::{
    DetectorInput, Hit, HitEdit, Registry, SourceUnit, excerpt_of, run_review,
};
use serde_json::{Value, json};

struct CountingEnv(Cell<u32>);
impl Env for CountingEnv {
    fn now(&self) -> String {
        "T".to_owned()
    }
    fn new_id(&self) -> String {
        self.0.set(self.0.get() + 1);
        format!("n{}", self.0.get())
    }
}

fn fixture() -> Value {
    let path = format!(
        "{}/tests/fixtures/detect-conformance.json",
        env!("CARGO_MANIFEST_DIR")
    );
    serde_json::from_str(&std::fs::read_to_string(path).expect("fixture")).expect("JSON")
}

fn utf16_col(line: &str, byte: usize) -> u32 {
    u32::try_from(line[..byte].encode_utf16().count()).unwrap() + 1
}

/// The same toy the generator registers on the TypeScript side.
fn toy(input: &DetectorInput<'_>, confidence: Confidence) -> Vec<Hit> {
    input
        .content
        .split('\n')
        .enumerate()
        .filter_map(|(i, line)| {
            let at = line.find("TOY")?;
            let start = utf16_col(line, at);
            Some(Hit {
                line: u32::try_from(i).unwrap() + 1,
                columns: LineRange {
                    start,
                    end: start + 3,
                },
                excerpt: line.trim().to_owned(),
                confidence,
                severity: line.contains("SEV").then_some(Severity::Critical),
                edit: line.contains("EDIT").then(|| HitEdit {
                    start_column: start,
                    end_column: start + 3,
                    replacement: "OK".to_owned(),
                    safety: EditSafety::Safe,
                }),
            })
        })
        .collect()
}
fn toy_high(i: &DetectorInput<'_>) -> Vec<Hit> {
    toy(i, Confidence::High)
}
fn toy_medium(i: &DetectorInput<'_>) -> Vec<Hit> {
    toy(i, Confidence::Medium)
}
fn toy_low(i: &DetectorInput<'_>) -> Vec<Hit> {
    toy(i, Confidence::Low)
}

fn registry() -> Registry {
    let mut r = Registry::new();
    r.register_text("toy-pack", toy_high);
    r.register_text("toy-criteria", toy_medium);
    r.register_text("toy-nocat", toy_low);
    r.register_text("eqeqeq", toy_high);
    r.register_text("no-var", toy_medium);
    r
}

fn units(fx: &Value) -> Vec<SourceUnit> {
    fx["units"]
        .as_array()
        .unwrap()
        .iter()
        .map(|u| SourceUnit {
            file: u["file"].as_str().unwrap().to_owned(),
            content: u["content"].as_str().unwrap().to_owned(),
            changed_lines: None,
        })
        .collect()
}

#[test]
fn excerpts_are_byte_identical_to_typescript() {
    let fx = fixture();
    for case in fx["excerpts"].as_array().unwrap() {
        let line = case["line"].as_str().unwrap();
        let want = case["excerpt"]
            .as_str()
            .expect("the TypeScript detector hit every line");
        // A cut that splits an astral character: TypeScript keeps a lone surrogate (written as
        // U+FFFD by the generator), Rust writes U+FFFD itself.
        assert_eq!(excerpt_of(line), want, "{line:?}");
    }
}

#[test]
fn the_findings_the_runner_builds_match_typescript() {
    let fx = fixture();
    let policy: ReviewPolicy = serde_json::from_value(fx["policy"].clone()).expect("policy");
    let run = run_review(
        &policy,
        &units(&fx),
        &registry(),
        &CountingEnv(Cell::new(0)),
    )
    .unwrap();
    let got: Vec<Value> = run
        .findings
        .iter()
        .map(|f| serde_json::to_value(f).unwrap())
        .collect();
    let want = fx["findings"].as_array().unwrap();
    assert_eq!(got.len(), want.len());
    for (g, w) in got.iter().zip(want) {
        assert_eq!(g, w);
    }
    let suppressed: Vec<Value> = run
        .suppressed
        .iter()
        .map(|s| {
            let mut v = json!({ "file": s.file, "line": s.line, "ruleId": s.rule_id });
            if let Some(r) = &s.reason {
                v["reason"] = json!(r);
            }
            v
        })
        .collect();
    assert_eq!(Value::Array(suppressed), fx["suppressed"]);
}

#[test]
fn a_diff_review_keeps_only_the_hits_on_added_lines() {
    let fx = fixture();
    let policy: ReviewPolicy = serde_json::from_value(fx["policy"].clone()).unwrap();
    let mut us = units(&fx);
    // `src/c.ts` has TOY on lines 2 and 3; only line 3 was added (and its file-wide directive
    // covers `no-var`, so use the rule that is not suppressed there).
    us.truncate(1);
    us[0].changed_lines = Some(BTreeSet::from([3]));
    let run = run_review(&policy, &us, &registry(), &CountingEnv(Cell::new(0))).unwrap();
    assert!(!run.findings.is_empty());
    for f in &run.findings {
        assert_eq!(f.locations[0].lines.unwrap().start, 3);
    }
}

#[test]
fn rules_without_a_detector_produce_nothing_and_say_nothing() {
    let fx = fixture();
    let policy: ReviewPolicy = serde_json::from_value(fx["policy"].clone()).unwrap();
    let run = run_review(
        &policy,
        &units(&fx),
        &Registry::new(),
        &CountingEnv(Cell::new(0)),
    )
    .unwrap();
    assert!(run.findings.is_empty() && run.suppressed.is_empty());
}

fn tree_only(_: &DetectorInput<'_>) -> Vec<Hit> {
    vec![]
}

#[test]
fn syntax_detectors_skip_files_they_cannot_read_and_say_so() {
    let fx = fixture();
    let policy: ReviewPolicy = serde_json::from_value(fx["policy"].clone()).unwrap();
    let mut r = Registry::new();
    r.register("toy-pack", tree_only);
    let us = vec![
        SourceUnit {
            file: "a.ts".into(),
            content: "const a = 1;\n".into(),
            changed_lines: None,
        },
        SourceUnit {
            file: "broken.ts".into(),
            content: "const a = `open".into(),
            changed_lines: None,
        },
        SourceUnit {
            file: "main.zig".into(),
            content: "pub fn main() void {}".into(),
            changed_lines: None,
        },
    ];
    let run = run_review(&policy, &us, &r, &CountingEnv(Cell::new(0))).unwrap();
    assert_eq!(run.not_analyzed.len(), 2);
    assert_eq!(
        run.not_analyzed["main.zig"],
        debuggatha_engine::detect::NotAnalyzed::NoParser
    );
    assert!(!run.not_analyzed.contains_key("a.ts"));
}

#[test]
fn a_text_detector_still_runs_on_a_file_that_failed_to_parse() {
    let fx = fixture();
    let policy: ReviewPolicy = serde_json::from_value(fx["policy"].clone()).unwrap();
    let mut r = Registry::new();
    r.register_text("toy-pack", toy_high);
    // Not valid TypeScript: no tree, so no comments are found and the directive cannot be read.
    // The finding is reported anyway: a suppression that cannot be read fails safe.
    let us = vec![SourceUnit {
        file: "broken.ts".into(),
        content: "// debuggatha-ignore toy-pack\nTOY !!\n".into(),
        changed_lines: None,
    }];
    let run = run_review(&policy, &us, &r, &CountingEnv(Cell::new(0))).unwrap();
    assert_eq!(run.findings.len(), 1);
    assert!(run.suppressed.is_empty());
}

/// The excerpts the TypeScript engine put in its 7,736 real findings are already what `excerpt_of`
/// produces, so applying it again must leave them alone (trim, redaction and the 200-unit cut are
/// idempotent). A cheap check on real text: astral characters, CRLF, tokens.
#[test]
fn real_excerpts_are_fixed_points_of_excerpt_of() {
    use std::io::Read;
    let dir = format!(
        "{}/../../corpus/golden/ts-findings",
        env!("CARGO_MANIFEST_DIR")
    );
    let mut n = 0;
    for entry in std::fs::read_dir(dir).unwrap() {
        let mut text = String::new();
        flate2::read::GzDecoder::new(std::fs::File::open(entry.unwrap().path()).unwrap())
            .read_to_string(&mut text)
            .unwrap();
        for f in serde_json::from_str::<Vec<Value>>(&text).unwrap() {
            for e in f["evidence"].as_array().unwrap() {
                if let Some(x) = e["excerpt"].as_str().filter(|_| e["kind"] == "code") {
                    assert_eq!(excerpt_of(x), x);
                    n += 1;
                }
            }
        }
    }
    assert!(n >= 7_000, "{n} excerpts");
}
