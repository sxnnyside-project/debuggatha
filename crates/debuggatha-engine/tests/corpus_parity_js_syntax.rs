//! The JavaScript and TypeScript syntax rules on the real-repository corpus against what the TypeScript engine reported
//! (opt-in: needs the clones, `node scripts/parity/fetch-corpus.mjs`).
//! Run: `DEBUGGATHA_CORPUS_DIR=<dir> cargo test -p debuggatha-engine --test corpus_parity_js_syntax --release -- --ignored --nocapture`
//!
//! For each repository the policy is the golden one (`corpus/golden/ts-context`), the files are the
//! ones the TypeScript engine scanned, and the findings of the ten syntax rules are compared by
//! (rule, file, line). Every difference is printed with its rule so it can be classified.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::print_stdout)] // a diagnostic test

use std::cell::Cell;
use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::path::Path;

use debuggatha_core::env::Env;
use debuggatha_core::knowledge_system::ReviewPolicy;
use debuggatha_engine::detect::{SourceUnit, default_registry, js, run_review};
use debuggatha_engine::scan_scope::{PARITY_EXTENSIONS, is_generated_content, list_scan_files};
use serde_json::Value;

struct Ids(Cell<u32>);
impl Env for Ids {
    fn now(&self) -> String {
        "T".into()
    }
    fn new_id(&self) -> String {
        self.0.set(self.0.get() + 1);
        format!("n{}", self.0.get())
    }
}

type Key = (String, String, u64);

fn golden(name: &str) -> (ReviewPolicy, BTreeMap<Key, Value>) {
    let root = format!("{}/../../corpus/golden", env!("CARGO_MANIFEST_DIR"));
    let ctx: Value = serde_json::from_str(
        &std::fs::read_to_string(format!("{root}/ts-context/{name}.json")).unwrap(),
    )
    .unwrap();
    let policy: ReviewPolicy = serde_json::from_value(ctx["policy"].clone()).unwrap();
    let mut text = String::new();
    flate2::read::GzDecoder::new(
        std::fs::File::open(format!("{root}/ts-findings/{name}.json.gz")).unwrap(),
    )
    .read_to_string(&mut text)
    .unwrap();
    let mut out = BTreeMap::new();
    for f in serde_json::from_str::<Vec<Value>>(&text).unwrap() {
        let rule = f["evidence"]
            .as_array()
            .unwrap()
            .iter()
            .find_map(|e| e["ruleId"].as_str())
            .unwrap_or("?")
            .to_owned();
        if js::RULES.contains(&rule.as_str()) {
            let loc = &f["locations"][0];
            out.insert(
                (
                    rule,
                    loc["file"].as_str().unwrap().to_owned(),
                    loc["lines"]["start"].as_u64().unwrap(),
                ),
                f,
            );
        }
    }
    (policy, out)
}

// One long scenario read top to bottom: split into helpers it would only be harder to follow.
#[allow(clippy::too_many_lines)]
#[test]
#[ignore = "needs the corpus clones: run it with its `just corpus-*` recipe"]
fn js_syntax_rules_against_the_golden_findings() {
    let Some(dir) = std::env::var_os("DEBUGGATHA_CORPUS_DIR") else {
        panic!("DEBUGGATHA_CORPUS_DIR must be set: run it with its `just corpus-*` recipe");
    };
    let names: Vec<String> = std::fs::read_dir(format!(
        "{}/../../corpus/golden/ts-context",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
    .map(|e| {
        e.unwrap()
            .path()
            .file_stem()
            .unwrap()
            .to_string_lossy()
            .into_owned()
    })
    .collect();
    let registry = default_registry();
    let mut table: BTreeMap<String, [u32; 4]> = BTreeMap::new(); // same, only ts, only rust, field differs
    let mut samples: BTreeMap<(String, &str), Vec<String>> = BTreeMap::new();

    for name in names {
        if std::env::var("DEBUGGATHA_ONLY").is_ok_and(|only| only != name) {
            continue;
        }
        let repo = Path::new(&dir).join(&name);
        if !repo.is_dir() {
            continue;
        }
        let (mut policy, want) = golden(&name);
        policy.rules.retain(|r| js::RULES.contains(&r.id.as_str()));
        let units: Vec<SourceUnit> = list_scan_files(&repo, PARITY_EXTENSIONS)
            .into_iter()
            .filter_map(|file| {
                let bytes = std::fs::read(repo.join(&file)).ok()?;
                let content = String::from_utf8_lossy(&bytes).into_owned();
                (!is_generated_content(&content)).then_some(SourceUnit {
                    file,
                    content,
                    changed_lines: None,
                })
            })
            .collect();
        let run = run_review(&policy, &units, &registry, &Ids(Cell::new(0))).unwrap();

        let mut got: BTreeMap<Key, Value> = BTreeMap::new();
        for f in &run.findings {
            let v = serde_json::to_value(f).unwrap();
            let rule = v["evidence"]
                .as_array()
                .unwrap()
                .iter()
                .find_map(|e| e["ruleId"].as_str())
                .unwrap()
                .to_owned();
            let loc = &v["locations"][0];
            got.insert(
                (
                    rule,
                    loc["file"].as_str().unwrap().to_owned(),
                    loc["lines"]["start"].as_u64().unwrap(),
                ),
                v,
            );
        }
        // Compare only the files Rust has a parser for and the rules it implements.
        let keys: BTreeSet<&Key> = want.keys().chain(got.keys()).collect();
        for k in keys {
            let row = table.entry(k.0.clone()).or_default();
            match (want.get(k), got.get(k)) {
                (Some(w), Some(g)) => {
                    let proj = |f: &Value| {
                        (
                            f["severity"].clone(),
                            f["confidence"].clone(),
                            f["category"].clone(),
                            f["locations"][0]["columns"].clone(),
                            f["evidence"][0]["excerpt"].clone(),
                        )
                    };
                    if proj(w) == proj(g) {
                        row[0] += 1;
                    } else {
                        row[3] += 1;
                        let note =
                            format!("{name}:{}:{} ts {:?} rust {:?}", k.1, k.2, proj(w), proj(g));
                        samples
                            .entry((k.0.clone(), "field"))
                            .or_default()
                            .push(note);
                    }
                }
                (Some(_), None) => {
                    row[1] += 1;
                    samples
                        .entry((k.0.clone(), "only-ts"))
                        .or_default()
                        .push(format!("{name}:{}:{}", k.1, k.2));
                }
                (None, Some(_)) => {
                    row[2] += 1;
                    samples
                        .entry((k.0.clone(), "only-rust"))
                        .or_default()
                        .push(format!("{name}:{}:{}", k.1, k.2));
                }
                (None, None) => {}
            }
        }
    }
    println!(
        "{:<32} {:>6} {:>8} {:>10} {:>7}",
        "rule", "same", "only ts", "only rust", "fields"
    );
    for (rule, [a, b, c, d]) in &table {
        println!("{rule:<32} {a:>6} {b:>8} {c:>10} {d:>7}");
    }

    // Every difference is classified in `docs/PARITY.md`. The committed golden holds the exact
    // set, so a new one (or a fixed one) fails here until it has been looked at.
    let current = serde_json::json!({
        "same": table.iter().map(|(r, v)| (r.clone(), v[0])).collect::<BTreeMap<_, _>>(),
        "differences": samples
            .iter()
            .map(|((r, k), v)| (format!("{r}/{k}"), v))
            .collect::<BTreeMap<_, _>>(),
    });
    let golden_path = format!(
        "{}/../../corpus/golden/divergences-js-syntax.json",
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
