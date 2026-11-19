//! The rules of the tree-sitter languages on the real-repository corpus against what the TypeScript detectors report
//! (opt-in: needs the clones, `node scripts/parity/fetch-corpus.mjs`, and the dump of the TypeScript hits).
//! Run:
//! ```text
//! bun scripts/parity/dump-detector-hits.ts <corpus> <ts-hits.json> <the five rule ids>
//! DEBUGGATHA_CORPUS_DIR=<corpus> DEBUGGATHA_TS_HITS=<ts-hits.json> \
//!   cargo test -p debuggatha-engine --test corpus_parity_tree_sitter_rules --release -- --nocapture
//! ```
//! Detector by detector, because the golden policies hold only some of these rules. Hits are
//! compared by (repository, rule, file, line) and then by columns, confidence and excerpt; the
//! exact set of differences is committed in `corpus/golden/divergences-tree-sitter-rules.json`.
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::print_stdout,
    clippy::case_sensitive_file_extension_comparisons // the corpus holds lower-case extensions
)] // a diagnostic test

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use debuggatha_engine::detect::{DetectorInput, concepts, queries};
use debuggatha_engine::scan_scope::{PARITY_EXTENSIONS, is_generated_content, list_scan_files};
use debuggatha_engine::syntax::parse;
use serde_json::Value;

type Key = (String, String, String, u64);
type Fields = (u64, u64, String, String);

// One long scenario read top to bottom: split into helpers it would only be harder to follow.
#[allow(clippy::too_many_lines)]
#[test]
#[ignore = "needs the corpus clones: run it with its `just corpus-*` recipe"]
fn tree_sitter_rules_against_the_typescript_detectors() {
    let (Some(dir), Some(dump)) = (
        std::env::var_os("DEBUGGATHA_CORPUS_DIR"),
        std::env::var_os("DEBUGGATHA_TS_HITS"),
    ) else {
        panic!(
            "DEBUGGATHA_CORPUS_DIR or DEBUGGATHA_TS_HITS must be set: run it with its `just corpus-*` recipe"
        );
    };
    let mut want: BTreeMap<Key, Fields> = BTreeMap::new();
    let rows: Vec<Value> = serde_json::from_str(&std::fs::read_to_string(dump).unwrap()).unwrap();
    for r in rows {
        // `no-eval` in JavaScript belongs to the JavaScript syntax rules; this answers for PHP.
        if r[1] == "no-eval" && !r[2].as_str().unwrap().ends_with(".php") {
            continue;
        }
        want.insert(
            (
                r[0].as_str().unwrap().to_owned(),
                r[1].as_str().unwrap().to_owned(),
                r[2].as_str().unwrap().to_owned(),
                r[3].as_u64().unwrap(),
            ),
            (
                r[4].as_u64().unwrap(),
                r[5].as_u64().unwrap(),
                r[6].as_str().unwrap().to_owned(),
                r[7].as_str().unwrap().to_owned(),
            ),
        );
    }

    let mut got: BTreeMap<Key, Fields> = BTreeMap::new();
    let mut repos: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.is_dir())
        .collect();
    repos.sort();
    for repo in repos {
        let name = repo.file_name().unwrap().to_string_lossy().into_owned();
        for file in list_scan_files(&repo, PARITY_EXTENSIONS) {
            let Ok(bytes) = std::fs::read(repo.join(&file)) else {
                continue;
            };
            let content = String::from_utf8_lossy(&bytes).into_owned();
            if is_generated_content(&content) {
                continue;
            }
            let Some(syntax) = parse(Path::new(&file), &content) else {
                continue;
            };
            let input = DetectorInput::new(&file, &content, Some(&syntax));
            for (rule, h) in queries::detect(&input)
                .into_iter()
                .chain(concepts::detect(&input))
            {
                got.insert(
                    (
                        name.clone(),
                        rule.to_owned(),
                        file.clone(),
                        u64::from(h.line),
                    ),
                    (
                        u64::from(h.columns.start),
                        u64::from(h.columns.end),
                        serde_json::to_value(h.confidence)
                            .unwrap()
                            .as_str()
                            .unwrap()
                            .to_owned(),
                        h.excerpt,
                    ),
                );
            }
        }
    }

    let mut table: BTreeMap<String, [u32; 4]> = BTreeMap::new(); // same, only ts, only rust, fields
    let mut samples: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for k in want.keys().chain(got.keys()).collect::<BTreeSet<_>>() {
        let row = table.entry(k.1.clone()).or_default();
        let at = format!("{}:{}:{}", k.0, k.2, k.3);
        match (want.get(k), got.get(k)) {
            (Some(w), Some(g)) if w == g => row[0] += 1,
            (Some(w), Some(g)) => {
                row[3] += 1;
                samples
                    .entry(format!("{}/field", k.1))
                    .or_default()
                    .push(format!("{at} ts {w:?} rust {g:?}"));
            }
            (Some(_), None) => {
                row[1] += 1;
                samples
                    .entry(format!("{}/only-ts", k.1))
                    .or_default()
                    .push(at);
            }
            (None, Some(_)) => {
                row[2] += 1;
                samples
                    .entry(format!("{}/only-rust", k.1))
                    .or_default()
                    .push(at);
            }
            (None, None) => {}
        }
    }
    println!(
        "{:<26} {:>6} {:>8} {:>10} {:>7}",
        "rule", "same", "only ts", "only rust", "fields"
    );
    for (rule, [a, b, c, d]) in &table {
        println!("{rule:<26} {a:>6} {b:>8} {c:>10} {d:>7}");
    }

    let current = serde_json::json!({
        "same": table.iter().map(|(r, v)| (r.clone(), v[0])).collect::<BTreeMap<_, _>>(),
        "differences": samples,
    });
    let golden_path = format!(
        "{}/../../corpus/golden/divergences-tree-sitter-rules.json",
        env!("CARGO_MANIFEST_DIR")
    );
    if std::env::var_os("UPDATE_GOLDENS").is_some() {
        let text = serde_json::to_string_pretty(&current).unwrap();
        std::fs::write(&golden_path, format!("{text}\n")).unwrap();
    }
    let golden: Value =
        serde_json::from_str(&std::fs::read_to_string(&golden_path).unwrap_or_default())
            .unwrap_or(Value::Null);
    assert_eq!(
        current, golden,
        "the differences changed: review them, then UPDATE_GOLDENS=1"
    );
}
