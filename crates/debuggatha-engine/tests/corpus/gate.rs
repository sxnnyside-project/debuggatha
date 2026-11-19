//! Detector gate: every detector group together, through the default registry and the golden
//! policy of each repository, against the 7,736 findings the TypeScript engine reported on the
//! corpus (`corpus/golden/ts-findings`). Opt-in: needs the clones (`node scripts/parity/fetch-corpus.mjs`).
//! Run: `DEBUGGATHA_CORPUS_DIR=<dir> cargo test -p debuggatha-engine --test corpus --release -- gate:: --ignored --nocapture`
//!
//! A finding is compared whole (title, explanation, severity, confidence, category, locations,
//! evidence, policy, recommendations) except its random `id`. The committed golden
//! (`corpus/golden/divergences-all-detectors.json`) holds the exact set of differences; each kind is
//! classified in `docs/PARITY.md`, so a new or a fixed difference fails until it has been looked
//! at. Findings of rules with no Rust detector are counted separately: they must all come from
//! the analysis, never from a detector that was left behind.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::print_stdout)] // a diagnostic test

use std::cell::Cell;
use std::collections::BTreeMap;
use std::io::Read;
use std::path::Path;

use debuggatha_core::env::Env;
use debuggatha_core::knowledge_system::ReviewPolicy;
use debuggatha_engine::detect::{NEW_RULES, SourceUnit, default_registry, run_review};
use debuggatha_engine::scan_scope::{PARITY_EXTENSIONS, is_generated_content, list_scan_files};
use serde_json::{Value, json};

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

/// (rule, file, line, nth finding of that rule on that line).
type Key = (String, String, u64, usize);

fn rule_of(f: &Value) -> String {
    f["evidence"]
        .as_array()
        .unwrap()
        .iter()
        .find_map(|e| e["ruleId"].as_str())
        .unwrap_or("?")
        .to_owned()
}

fn keyed(findings: Vec<Value>) -> Vec<(Key, Value)> {
    let mut seen: BTreeMap<(String, String, u64), usize> = BTreeMap::new();
    findings
        .into_iter()
        .map(|mut f| {
            let rule = rule_of(&f);
            let loc = &f["locations"][0];
            let (file, line) = (
                loc["file"].as_str().unwrap().to_owned(),
                loc["lines"]["start"].as_u64().unwrap(),
            );
            // `id` is random. `appliedPolicy` is checked apart (see `policy_ids`): the goldens were
            // reviewed under another policy id than the one stored in `ts-context`.
            f.as_object_mut().unwrap().remove("id");
            f.as_object_mut().unwrap().remove("appliedPolicy");
            let n = seen.entry((rule.clone(), file.clone(), line)).or_default();
            *n += 1;
            ((rule, file, line, *n), f)
        })
        .collect()
}

fn golden(name: &str) -> (ReviewPolicy, Vec<Value>) {
    let root = format!("{}/../../corpus/golden", env!("CARGO_MANIFEST_DIR"));
    let ctx: Value = serde_json::from_str(
        &std::fs::read_to_string(format!("{root}/ts-context/{name}.json")).unwrap(),
    )
    .unwrap();
    let mut text = String::new();
    flate2::read::GzDecoder::new(
        std::fs::File::open(format!("{root}/ts-findings/{name}.json.gz")).unwrap(),
    )
    .read_to_string(&mut text)
    .unwrap();
    (
        serde_json::from_value(ctx["policy"].clone()).unwrap(),
        serde_json::from_str(&text).unwrap(),
    )
}

/// The fields that differ between two findings, by name.
fn differing_fields(a: &Value, b: &Value) -> Vec<String> {
    let (a, b) = (a.as_object().unwrap(), b.as_object().unwrap());
    let mut keys: Vec<&String> = a.keys().chain(b.keys()).collect();
    keys.sort();
    keys.dedup();
    keys.into_iter()
        .filter(|k| a.get(*k) != b.get(*k))
        .cloned()
        .collect()
}

#[derive(Default)]
struct Row {
    same: u32,
    only_ts: u32,
    only_rust: u32,
    fields: u32,
}

// One long scenario read top to bottom: split into helpers it would only be harder to follow.
#[allow(clippy::too_many_lines)]
#[test]
#[ignore = "needs the corpus clones: run it with its `just corpus-*` recipe"]
fn every_detector_together_against_the_golden_findings() {
    let Some(dir) = std::env::var_os("DEBUGGATHA_CORPUS_DIR") else {
        panic!("DEBUGGATHA_CORPUS_DIR must be set: run it with its `just corpus-*` recipe");
    };
    let root = format!("{}/../../corpus/golden", env!("CARGO_MANIFEST_DIR"));
    let mut names: Vec<String> = std::fs::read_dir(format!("{root}/ts-context"))
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
    names.sort();
    let registry = default_registry();
    let mut table: BTreeMap<String, Row> = BTreeMap::new();
    let mut samples: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut new_rules: BTreeMap<String, u32> = BTreeMap::new(); // rules the TypeScript engine does not have
    let mut outside: BTreeMap<String, u32> = BTreeMap::new(); // golden findings of rules with no detector
    let (mut repos, mut order_breaks, mut suppressed) = (0, 0, 0);
    // Every Rust finding, for the false-positive sample (`DEBUGGATHA_SAMPLES`).
    let mut all_hits: BTreeMap<String, Vec<Value>> = BTreeMap::new();

    for name in names {
        let repo = Path::new(&dir).join(&name);
        // A missing or half-fetched clone would turn into "TypeScript only" differences, which is
        // a broken corpus, not a divergence: stop here.
        assert!(
            repo.join(".git").is_dir() && repo.read_dir().unwrap().count() > 1,
            "{name} is missing or empty: node scripts/parity/fetch-corpus.mjs <dir>"
        );
        repos += 1;
        let (policy, golden_findings) = golden(&name);
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
        // Every golden finding names one policy; the Rust findings name the policy they ran under.
        let mut golden_policies: Vec<&str> = golden_findings
            .iter()
            .filter_map(|f| f["appliedPolicy"]["id"].as_str())
            .collect();
        golden_policies.sort_unstable();
        golden_policies.dedup();
        assert!(golden_policies.len() <= 1, "{name}: {golden_policies:?}");
        let run = run_review(&policy, &units, &registry, &Ids(Cell::new(0))).unwrap();
        suppressed += run.suppressed.len();
        assert!(
            run.findings
                .iter()
                .all(|f| serde_json::to_value(f).unwrap()["appliedPolicy"]["id"] == policy.id),
            "{name}: a finding names another policy"
        );

        let mut want = Vec::new();
        for f in golden_findings {
            if registry.has(&rule_of(&f)) {
                want.push(f);
            } else {
                *outside.entry(rule_of(&f)).or_default() += 1;
            }
        }
        for f in &run.findings {
            let v = serde_json::to_value(f).unwrap();
            let (file, line) = (
                v["locations"][0]["file"].as_str().unwrap().to_owned(),
                v["locations"][0]["lines"]["start"].as_u64().unwrap(),
            );
            let around = units
                .iter()
                .find(|u| u.file == file)
                .map(|u| {
                    let first = usize::try_from(line).unwrap().saturating_sub(2);
                    u.content
                        .split('\n')
                        .skip(first)
                        .take(3)
                        .collect::<Vec<_>>()
                        .join("\n")
                })
                .unwrap_or_default();
            all_hits.entry(rule_of(&v)).or_default().push(json!({
                "repo": name, "file": file, "line": line, "confidence": v["confidence"],
                "excerpt": v["evidence"][0]["excerpt"], "around": around,
            }));
        }
        let want = keyed(want);
        let mut ours = Vec::new();
        for f in &run.findings {
            let v = serde_json::to_value(f).unwrap();
            if NEW_RULES.contains(&rule_of(&v).as_str()) {
                *new_rules.entry(rule_of(&v)).or_default() += 1;
            } else {
                ours.push(v);
            }
        }
        let got = keyed(ours);
        let got_by: BTreeMap<&Key, &Value> = got.iter().map(|(k, v)| (k, v)).collect();
        let want_by: BTreeMap<&Key, &Value> = want.iter().map(|(k, v)| (k, v)).collect();

        for (k, w) in &want {
            let row = table.entry(k.0.clone()).or_default();
            match got_by.get(k) {
                Some(g) if g == &w => row.same += 1,
                Some(g) => {
                    row.fields += 1;
                    samples
                        .entry(format!("{}/fields", k.0))
                        .or_default()
                        .push(format!(
                            "{name}:{}:{}#{} {}",
                            k.1,
                            k.2,
                            k.3,
                            differing_fields(w, g).join(",")
                        ));
                }
                None => {
                    row.only_ts += 1;
                    samples
                        .entry(format!("{}/only-ts", k.0))
                        .or_default()
                        .push(format!("{name}:{}:{}#{}", k.1, k.2, k.3));
                }
            }
        }
        for (k, _) in &got {
            if !want_by.contains_key(k) {
                table.entry(k.0.clone()).or_default().only_rust += 1;
                samples
                    .entry(format!("{}/only-rust", k.0))
                    .or_default()
                    .push(format!("{name}:{}:{}#{}", k.1, k.2, k.3));
            }
        }
        // Order: the findings both engines report come out in the same sequence.
        let common = |list: &[(Key, Value)], other: &BTreeMap<&Key, &Value>| -> Vec<Key> {
            list.iter()
                .filter(|(k, _)| other.contains_key(k))
                .map(|(k, _)| k.clone())
                .collect()
        };
        if common(&want, &got_by) != common(&got, &want_by) {
            order_breaks += 1;
        }
    }

    println!("{repos} repositories");
    println!(
        "{:<28} {:>6} {:>8} {:>10} {:>7}",
        "rule", "same", "only ts", "only rust", "fields"
    );
    for (rule, r) in &table {
        println!(
            "{rule:<28} {:>6} {:>8} {:>10} {:>7}",
            r.same, r.only_ts, r.only_rust, r.fields
        );
    }
    println!("findings of the rules the TypeScript engine does not have: {new_rules:?}");
    println!("golden findings of rules with no detector: {outside:?}");
    println!(
        "inline suppressions: {suppressed}, repositories with a different order: {order_breaks}"
    );

    // A deterministic sample: up to 50 evenly spaced findings of each rule, in scan order.
    if let Some(path) = std::env::var_os("DEBUGGATHA_SAMPLES") {
        let sample: BTreeMap<&String, Value> = all_hits
            .iter()
            .map(|(rule, hits)| {
                let cap = std::env::var("DEBUGGATHA_SAMPLE_MAX")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(50);
                let step = hits.len().div_ceil(cap).max(1);
                let picked: Vec<&Value> = hits.iter().step_by(step).take(cap).collect();
                (rule, json!({ "total": hits.len(), "sample": picked }))
            })
            .collect();
        std::fs::write(path, serde_json::to_string_pretty(&sample).unwrap()).unwrap();
    }

    let current = json!({
        "repositories": repos,
        "rules": table.iter().map(|(r, v)| (r.clone(), json!({
            "same": v.same, "onlyTs": v.only_ts, "onlyRust": v.only_rust, "fields": v.fields,
        }))).collect::<BTreeMap<_, _>>(),
        "newRules": new_rules,
        "withoutDetector": outside,
        "ordersDiffering": order_breaks,
        "differences": samples,
    });
    // Every difference must already be classified: it has to appear in the golden of the detector
    // group that owns the rule (the groups compare against the full TypeScript hits, which hold
    // more than the golden findings do), so a difference nobody looked at cannot hide here.
    let mut classified: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for group in [
        "js-syntax",
        "js-calls",
        "tree-sitter-rules",
        "vue-template",
        "secrets",
    ] {
        let text = std::fs::read_to_string(format!("{root}/divergences-{group}.json")).unwrap();
        let golden: Value = serde_json::from_str(&text).unwrap();
        for (kind, list) in golden["differences"].as_object().unwrap() {
            classified.entry(kind.clone()).or_default().extend(
                list.as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_str().unwrap().to_owned()),
            );
        }
    }
    for (kind, list) in &samples {
        for item in list {
            let bare = item.rsplit_once('#').map_or(item.as_str(), |(a, _)| a);
            assert!(
                classified
                    .get(kind)
                    .is_some_and(|l| l.iter().any(|c| c == bare)),
                "{kind} {item} is in no group golden"
            );
        }
    }
    let golden_path = format!("{root}/divergences-all-detectors.json");
    if std::env::var_os("UPDATE_GOLDENS").is_some() {
        std::fs::write(
            &golden_path,
            format!("{}\n", serde_json::to_string_pretty(&current).unwrap()),
        )
        .unwrap();
    }
    let committed: Value =
        serde_json::from_str(&std::fs::read_to_string(&golden_path).unwrap_or_default())
            .unwrap_or(Value::Null);
    assert_eq!(
        current, committed,
        "the differences changed: review them, then UPDATE_GOLDENS=1"
    );
}
