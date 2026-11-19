//! Parse every JavaScript and TypeScript file of the real-repository corpus and report how the
//! parser did (opt-in: needs the clones, `node scripts/parity/fetch-corpus.mjs`).
//! Run: `DEBUGGATHA_CORPUS_DIR=<dir> cargo test -p debuggatha-engine --test corpus -- syntax_trees:: --ignored --nocapture`
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::print_stdout)] // a diagnostic test

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use debuggatha_engine::syntax::{ParseStatus, parse};

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let path = e.path();
        let name = e.file_name();
        if name == ".git" || name == "node_modules" {
            continue;
        }
        if e.file_type().is_ok_and(|t| t.is_dir()) {
            walk(&path, out);
        } else {
            out.push(path);
        }
    }
}

#[test]
#[ignore = "needs the corpus clones: run it with its `just corpus-*` recipe"]
fn parse_rate_on_the_corpus() {
    let Some(dir) = std::env::var_os("DEBUGGATHA_CORPUS_DIR") else {
        panic!("DEBUGGATHA_CORPUS_DIR must be set: run it with its `just corpus-*` recipe");
    };
    let mut files = Vec::new();
    walk(Path::new(&dir), &mut files);
    let (mut total, mut clean, mut with_errors, mut failed) = (0, 0, 0, 0);
    let mut per_language: BTreeMap<&str, [u32; 3]> = BTreeMap::new();
    let mut bad = Vec::new();
    for f in files {
        let Ok(source) = std::fs::read_to_string(&f) else {
            continue;
        };
        let Some(s) = parse(&f, &source) else {
            continue;
        };
        total += 1;
        let row = per_language.entry(s.language.id()).or_default();
        match s.status {
            ParseStatus::Parsed => {
                clean += 1;
                row[0] += 1;
            }
            ParseStatus::ParsedWithErrors => {
                with_errors += 1;
                row[1] += 1;
            }
            ParseStatus::Failed => {
                failed += 1;
                row[2] += 1;
            }
        }
        if s.status != ParseStatus::Parsed && bad.len() < 15 {
            bad.push(f.display().to_string());
        }
    }
    println!("files {total}: clean {clean}, with errors {with_errors}, failed {failed}");
    for (lang, [c, e, f]) in &per_language {
        let n = c + e + f;
        println!(
            "  {lang:<11} {n:>6} files: {:>5.1}% clean, {e} with errors, {f} failed",
            f64::from(*c) * 100.0 / f64::from(n)
        );
    }
    for b in bad {
        println!("  {b}");
    }
    assert!(total > 0);
}
