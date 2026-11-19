//! Measures a local model on the labeled set of the semantic layer.
//!
//! `just semantic-eval` (Ollama), or `cargo run --release -p debuggatha-engine --example
//! semantic_eval -- --runtime lmstudio --model <name> [--url <loopback url>] [--verbose]`.
//!
//! Prints per-category precision and recall and each verdict a person may want to read. The marks
//! are reference points for lightweight models, not a gate: doubting a real finding hides a real
//! problem, so verification is judged on not being wrong when a model says "false positive", and a
//! small model will not detect much, so detection is judged mostly on not raising false alarms.
#![allow(clippy::print_stdout, clippy::print_stderr)] // a command-line tool

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::path::Path;

use debuggatha_core::env::SystemEnv;
use debuggatha_core::review_engine::{
    Finding, ReviewDepth, SemanticVerdict, rule_id_from_evidence,
};
use debuggatha_engine::pipeline::{ReviewInput, Scope, execute_review};
use debuggatha_engine::semantic_layer::{
    CodeLine, CodeView, LocalRuntime, SemanticProvider, connect_local, detection_prompt,
    detections_from, judge, window_around,
};
use serde::Deserialize;

const DOUBT_PRECISION: f64 = 0.8;
const REAL_RETAINED: f64 = 0.9;
const DETECTION_PRECISION: f64 = 0.5;

#[derive(Deserialize)]
struct Dataset {
    verification: Vec<VerificationCase>,
    detection: Vec<DetectionCase>,
}

#[derive(Deserialize)]
struct VerificationCase {
    id: String,
    stack: String,
    file: String,
    rule: String,
    flagged: String,
    truth: String,
    code: String,
}

#[derive(Deserialize)]
struct Planted {
    at: String,
    category: String,
}

#[derive(Deserialize)]
struct DetectionCase {
    file: String,
    planted: Vec<Planted>,
    code: String,
}

fn manifest(stack: &str) -> (&'static str, &'static str) {
    match stack {
        "python" => ("requirements.txt", "flask\n"),
        "rust" => (
            "Cargo.toml",
            "[package]\nname = \"fixture\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        ),
        "go" => ("go.mod", "module fixture\n\ngo 1.22\n"),
        _ => (
            "package.json",
            r#"{"name":"fixture","dependencies":{"react":"^18.0.0","express":"^4.0.0"},"devDependencies":{"typescript":"^5.0.0"}}"#,
        ),
    }
}

fn make_repo(case: &VerificationCase) -> std::io::Result<tempdir::Dir> {
    let dir = tempdir::Dir::new()?;
    let (name, content) = manifest(&case.stack);
    std::fs::write(dir.path().join(name), content)?;
    let file = dir.path().join(&case.file);
    if let Some(parent) = file.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(file, &case.code)?;
    Ok(dir)
}

/// A directory that goes away with the value.
mod tempdir {
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU32, Ordering};

    static COUNT: AtomicU32 = AtomicU32::new(0);

    pub(crate) struct Dir(PathBuf);

    impl Dir {
        pub(crate) fn new() -> std::io::Result<Self> {
            let path = std::env::temp_dir().join(format!(
                "debuggatha-semantic-eval-{}-{}",
                std::process::id(),
                COUNT.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir_all(&path)?;
            Ok(Self(path))
        }

        pub(crate) fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for Dir {
        fn drop(&mut self) {
            drop(std::fs::remove_dir_all(&self.0));
        }
    }
}

/// The finding one of the built-in detectors makes on a case: the real input of verification.
fn built_in_finding(case: &VerificationCase, root: &Path) -> Option<Finding> {
    let input = ReviewInput {
        root,
        scope: Scope::Files(vec![case.file.clone()]),
        depth: ReviewDepth::Full,
        pack_ids: Vec::new(),
        policy_id: None,
        source_name: "semantic-eval",
        persist: false,
        include_baselined: false,
        analyzers: None,
        analyzer_search: Vec::new(),
        semantic: None,
    };
    let output = execute_review(&input, &SystemEnv).ok()?;
    let lines: Vec<&str> = case.code.split('\n').collect();
    output.result.findings.into_iter().find(|finding| {
        if rule_id_from_evidence(&finding.evidence).as_deref() != Some(case.rule.as_str()) {
            return false;
        }
        finding
            .locations
            .first()
            .and_then(|l| l.lines)
            .and_then(|l| lines.get((l.start as usize).checked_sub(1)?))
            .is_some_and(|text| text.contains(&case.flagged))
    })
}

struct VerificationOutcome {
    id: String,
    category: String,
    real: bool,
    verdict: String,
    reason: String,
}

fn evaluate_verification(
    provider: &dyn SemanticProvider,
    cases: &[VerificationCase],
) -> Vec<VerificationOutcome> {
    cases
        .iter()
        .map(|case| {
            let outcome = |category: &str, verdict: &str, reason: String| VerificationOutcome {
                id: case.id.clone(),
                category: category.to_owned(),
                real: case.truth == "real",
                verdict: verdict.to_owned(),
                reason,
            };
            let Ok(dir) = make_repo(case) else {
                return outcome("unknown", "failed", "could not write the case".to_owned());
            };
            let Some(finding) = built_in_finding(case, dir.path()) else {
                return outcome("unknown", "no-finding", "no detector finding".to_owned());
            };
            let category = finding.category.clone();
            let line = finding
                .locations
                .first()
                .and_then(|l| l.lines)
                .map_or(1, |l| l.start);
            let Some(view) = window_around(dir.path(), &case.file, line, 15) else {
                return outcome(&category, "failed", "file missing".to_owned());
            };
            match judge(provider, &finding, &view, &BTreeSet::new()) {
                Ok(Some(v)) => {
                    let verdict = match v.verdict {
                        SemanticVerdict::Confirmed => "confirmed",
                        SemanticVerdict::Doubtful => "doubtful",
                        SemanticVerdict::Unsure => "unsure",
                    };
                    outcome(&category, verdict, v.reason)
                }
                Ok(None) => outcome(&category, "failed", "unusable answer".to_owned()),
                Err(error) => outcome(&category, "failed", error),
            }
        })
        .collect()
}

#[derive(Default)]
struct Scores {
    cases: usize,
    real: usize,
    false_positive: usize,
    confirmed: usize,
    doubtful: usize,
    unsure: usize,
    failed: usize,
    doubted_right: usize,
    real_doubted: usize,
}

fn ratio(part: usize, whole: usize) -> Option<f64> {
    #[allow(clippy::cast_precision_loss)] // counts of a few dozen cases
    (whole != 0).then(|| part as f64 / whole as f64)
}

fn score<'a>(outcomes: impl Iterator<Item = &'a VerificationOutcome>) -> Scores {
    let mut s = Scores::default();
    for o in outcomes {
        s.cases += 1;
        if o.real {
            s.real += 1;
        } else {
            s.false_positive += 1;
        }
        match o.verdict.as_str() {
            "confirmed" => s.confirmed += 1,
            "doubtful" => {
                s.doubtful += 1;
                if o.real {
                    s.real_doubted += 1;
                } else {
                    s.doubted_right += 1;
                }
            }
            "unsure" => s.unsure += 1,
            _ => s.failed += 1,
        }
    }
    s
}

fn pct(value: Option<f64>) -> String {
    value.map_or_else(|| "  n/a".to_owned(), |v| format!("{:>3.0}%", v * 100.0))
}

#[derive(Default)]
struct Detection {
    planted: BTreeMap<String, (usize, usize)>,
    raised: usize,
    on_planted: usize,
    ungrounded: usize,
    failed: usize,
    clean_file_alarms: usize,
}

fn evaluate_detection(provider: &dyn SemanticProvider, cases: &[DetectionCase]) -> Detection {
    let mut total = Detection::default();
    for case in cases {
        let lines: Vec<&str> = case.code.split('\n').collect();
        let view = CodeView {
            file: case.file.clone(),
            lines: lines
                .iter()
                .enumerate()
                .map(|(i, text)| CodeLine {
                    n: u32::try_from(i + 1).unwrap_or(u32::MAX),
                    text: (*text).to_owned(),
                })
                .collect(),
        };
        let planted: Vec<(&str, i64)> = case
            .planted
            .iter()
            .map(|p| {
                let line = lines
                    .iter()
                    .position(|t| t.contains(&p.at))
                    .map_or(0, |i| i + 1);
                (p.category.as_str(), i64::try_from(line).unwrap_or(0))
            })
            .collect();
        let reply = provider.complete(&detection_prompt(&view, &BTreeSet::new()));
        let accepted = reply
            .ok()
            .and_then(|text| detections_from(&text, &view, provider, "eval", &SystemEnv).ok());
        let Some(found) = accepted else {
            total.failed += 1;
            for (category, _) in &planted {
                total.planted.entry((*category).to_owned()).or_default().0 += 1;
            }
            continue;
        };
        let at = |f: &Finding| {
            f.locations
                .first()
                .and_then(|l| l.lines)
                .map_or(-99, |l| i64::from(l.start))
        };
        for (category, line) in &planted {
            let entry = total.planted.entry((*category).to_owned()).or_default();
            entry.0 += 1;
            if found.accepted.iter().any(|f| (at(f) - line).abs() <= 2) {
                entry.1 += 1;
            }
        }
        total.raised += found.accepted.len();
        total.on_planted += found
            .accepted
            .iter()
            .filter(|f| planted.iter().any(|(_, line)| (line - at(f)).abs() <= 2))
            .count();
        total.ungrounded += found.rejected as usize;
        if planted.is_empty() {
            total.clean_file_alarms += found.accepted.len();
        }
    }
    total
}

fn report(model: &str, verification: &[VerificationOutcome], detection: &Detection) -> String {
    let mut out = vec![
        format!("Model: {model}"),
        String::new(),
        "Verification: does the model recognize false positives without doubting real findings?"
            .to_owned(),
        "category          cases real  fp  | confirmed doubtful unsure failed | doubt-precision fp-recall real-retained"
            .to_owned(),
    ];
    let categories: BTreeSet<&str> = verification.iter().map(|o| o.category.as_str()).collect();
    let overall = score(verification.iter());
    let rows = std::iter::once(("overall", overall)).chain(
        categories
            .iter()
            .map(|c| (*c, score(verification.iter().filter(|o| o.category == *c)))),
    );
    let mut overall_marks = (None, None);
    for (name, s) in rows {
        let doubt_precision = ratio(s.doubted_right, s.doubtful);
        let real_retained = ratio(s.real - s.real_doubted, s.real);
        if name == "overall" {
            overall_marks = (doubt_precision, real_retained);
        }
        out.push(format!(
            "{name:<17} {:>5} {:>4} {:>3}  | {:>9} {:>8} {:>6} {:>6} | {:>15} {:>9} {:>13}",
            s.cases,
            s.real,
            s.false_positive,
            s.confirmed,
            s.doubtful,
            s.unsure,
            s.failed,
            pct(doubt_precision),
            pct(ratio(s.doubted_right, s.false_positive)),
            pct(real_retained),
        ));
    }
    let reached = overall_marks.0.unwrap_or(1.0) >= DOUBT_PRECISION
        && overall_marks.1.unwrap_or(1.0) >= REAL_RETAINED;
    out.push(format!(
        "Benchmark: doubt-precision >= {:.0}% and real-retained >= {:.0}%  ->  {}",
        DOUBT_PRECISION * 100.0,
        REAL_RETAINED * 100.0,
        if reached { "reached" } else { "below" }
    ));
    let planted: usize = detection.planted.values().map(|v| v.0).sum();
    let found: usize = detection.planted.values().map(|v| v.1).sum();
    let precision = ratio(detection.on_planted, detection.raised);
    out.push(String::new());
    out.push("Detection: what a model finds in code the analyzers passed.".to_owned());
    out.push(format!(
        "planted {planted}, found {found} (recall {}), raised {}, on a planted defect {} (precision {}), false alarms {} ({} in clean files), ungrounded and dropped {}, failed calls {}",
        pct(ratio(found, planted)).trim(),
        detection.raised,
        detection.on_planted,
        pct(precision).trim(),
        detection.raised - detection.on_planted,
        detection.clean_file_alarms,
        detection.ungrounded,
        detection.failed,
    ));
    for (category, (planted, found)) in &detection.planted {
        out.push(format!(
            "  {category:<15} planted {planted}, found {found} (recall {})",
            pct(ratio(*found, *planted)).trim()
        ));
    }
    out.push(format!(
        "Benchmark: precision >= {:.0}%  ->  {}",
        DETECTION_PRECISION * 100.0,
        if precision.unwrap_or(1.0) >= DETECTION_PRECISION {
            "reached"
        } else {
            "below"
        }
    ));
    out.join("\n")
}

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let option = |name: &str| {
        args.iter()
            .position(|a| *a == format!("--{name}"))
            .and_then(|i| args.get(i + 1))
            .map(String::as_str)
    };
    let runtime = match option("runtime").unwrap_or("ollama") {
        "ollama" => LocalRuntime::Ollama,
        "lmstudio" => LocalRuntime::LmStudio,
        other => {
            return Err(format!(
                "Unknown runtime \"{other}\". Use ollama or lmstudio."
            ));
        }
    };
    let provider = connect_local(runtime, option("url"), option("model"), None)?;
    let name = format!(
        "{} / {}",
        provider.name(),
        provider.model().unwrap_or_default()
    );
    eprintln!("Evaluating {name} ...");

    let dataset: Dataset = serde_json::from_str(include_str!("../data/semantic-eval.json"))
        .map_err(|e| e.to_string())?;
    let verification = evaluate_verification(&provider, &dataset.verification);
    let detection = evaluate_detection(&provider, &dataset.detection);
    println!("{}", report(&name, &verification, &detection));
    if args.iter().any(|a| a == "--verbose") {
        println!("\nVerdicts:");
        for o in &verification {
            let right = if o.real == (o.verdict == "confirmed") {
                "ok "
            } else {
                "-- "
            };
            println!(
                "  {right}{:<34} truth={:<14} verdict={:<9} {}",
                o.id,
                if o.real { "real" } else { "false_positive" },
                o.verdict,
                o.reason
            );
        }
    }
    Ok(())
}
