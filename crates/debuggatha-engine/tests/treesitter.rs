//! The tree-sitter languages: every grammar loads and parses a sample cleanly, the masked text
//! hides comments and string text but keeps code (interpolations included), parse errors are
//! reported with their region, and the query runner finds what a query names.
//!
//! The masked samples are goldens of this engine, reviewed by hand (the TypeScript lexer does not read these languages:
//! `lex.ts` has no notion of an interpolation). Regenerate with
//! `UPDATE_GOLDENS=1 cargo test -p debuggatha-engine --test treesitter`.
#![allow(clippy::expect_used, clippy::unwrap_used)] // a failed expect is a failed test

use std::path::Path;

use debuggatha_engine::syntax::{
    Backend, Language, ParseStatus, Query, QueryError, Span, mask_source, parse,
};
use serde_json::{Value, json};

const SAMPLES: &[(&str, &str)] = &[
    (
        "a.rs",
        r##"// c
let s = "a\nb"; let c = 'x'; let r = r#"q"#; /* b */ env::var("X");
"##,
    ),
    (
        "a.py",
        r#"# c
s = f"a {os.environ['X']} b"
t = '''doc "q"'''
os.getenv('Y')
"#,
    ),
    (
        "a.go",
        r#"// c
s := "a\nb"
r := `raw`
os.Getenv("X")
"#,
    ),
    (
        "a.php",
        r#"<?php
// c
$s = "a $x b"; $t = 'it';
getenv('X');
"#,
    ),
    (
        "A.java",
        r#"// c
class A { String s = "a\n"; char c = 'x'; String t = System.getenv("X"); }
"#,
    ),
    (
        "a.cs",
        r#"// c
class A { string s = "a"; string i = $"x {Env.Get("Y")} z"; }
"#,
    ),
    (
        "a.rb",
        r#"# c
s = "a #{ENV['X']} b"
t = 'it'
"#,
    ),
    (
        "a.kt",
        r#"// c
val s = "a $x b ${System.getenv("Y")} c"
val t = System.getenv("X")
val m = """raw "q" ${y}"""
"#,
    ),
    (
        "a.swift",
        r#"// c
let s = "a \(x) b"
let t = ProcessInfo.processInfo.environment["X"]
"#,
    ),
    (
        "a.dart",
        r"// c
void f() {
  var s = 'a ${x} b';
  var r = r'raw';
  Platform.environment['X'];
}
",
    ),
    (
        "a.ex",
        r#"# c
s = "a #{x} b"
System.get_env("X")
h = """
here System.get_env("Z") #{w}
"""
"#,
    ),
    (
        "a.cpp",
        r#"// c
const char* s = "a\n"; char c = 'x'; auto r = R"(raw)"; getenv("X");
"#,
    ),
    (
        "a.gd",
        r#"# c
var s = "a"
var t = 'b'
OS.get_environment("X")
"#,
    ),
    (
        "a.html",
        r#"<!-- c -->
<a href="x.html">t</a>
"#,
    ),
];

fn golden_path() -> String {
    format!(
        "{}/tests/fixtures/treesitter-masks.json",
        env!("CARGO_MANIFEST_DIR")
    )
}

fn masked(file: &str, src: &str) -> String {
    let s = parse(Path::new(file), src).expect("a supported language");
    assert_eq!(s.status, ParseStatus::Parsed, "{file} must parse cleanly");
    mask_source(src, &s.facts.expect("facts"))
}

#[test]
fn every_language_masks_as_reviewed() {
    let got: Vec<Value> = SAMPLES
        .iter()
        .map(|(f, src)| json!({ "file": f, "source": src, "masked": masked(f, src) }))
        .collect();
    if std::env::var_os("UPDATE_GOLDENS").is_some() {
        let text = serde_json::to_string_pretty(&got).unwrap();
        std::fs::write(golden_path(), format!("{text}\n")).unwrap();
    }
    let want: Vec<Value> =
        serde_json::from_str(&std::fs::read_to_string(golden_path()).expect("golden")).unwrap();
    assert_eq!(got, want);
}

#[test]
fn the_masked_text_keeps_the_layout() {
    for (f, src) in SAMPLES {
        let m = masked(f, src);
        assert_eq!(m.encode_utf16().count(), src.encode_utf16().count(), "{f}");
        assert_eq!(m.lines().count(), src.lines().count(), "{f}");
    }
}

#[test]
fn all_tree_sitter_languages_are_covered_by_a_sample() {
    use std::collections::BTreeSet;
    let sampled: BTreeSet<_> = SAMPLES
        .iter()
        .filter_map(|(f, _)| Language::from_path(Path::new(f)))
        .collect();
    for l in Language::ALL {
        if l.backend() == Backend::TreeSitter {
            assert!(sampled.contains(&l), "{} has no sample", l.id());
        }
    }
}

#[test]
fn a_file_with_errors_is_reported_with_its_region_and_keeps_the_rest() {
    let src = "fn broken( {\n}\nfn ok() { let _ = env::var(\"X\"); }\n";
    let s = parse(Path::new("a.rs"), src).unwrap();
    assert_eq!(s.status, ParseStatus::ParsedWithErrors);
    assert!(s.errors >= 1 && !s.error_spans.is_empty());
    assert!(s.facts.is_some());
    let ok_at = u32::try_from(src.find("env::var").unwrap()).unwrap();
    assert!(
        !s.touches_error(Span::new(ok_at, ok_at + 8)),
        "the intact function is not an error region"
    );
}

#[test]
fn queries_find_what_they_name_and_nothing_inside_comments_or_strings() {
    let src = r#"// std::env::var("A")
fn f() { let s = "std::env::var(B)"; let _ = std::env::var("C"); }
"#;
    let s = parse(Path::new("a.rs"), src).unwrap();
    let q = Query::new(
        Language::Rust,
        r#"(call_expression function: (scoped_identifier name: (identifier) @n) (#eq? @n "var")) @call"#,
    )
    .unwrap();
    let matches = q.run(&s, src).unwrap();
    assert_eq!(
        matches.len(),
        1,
        "only the real call, not the comment or the string"
    );
    let call = matches[0]
        .captures
        .iter()
        .find(|c| c.name == "call")
        .unwrap();
    assert!(src[call.span.start as usize..call.span.end as usize].contains("\"C\""));
}

#[test]
fn bad_queries_and_wrong_files_are_errors_not_silence() {
    assert!(matches!(
        Query::new(Language::Rust, "(no_such_node) @x"),
        Err(QueryError::Invalid { .. })
    ));
    assert!(matches!(
        Query::new(Language::TypeScript, "(identifier) @x"),
        Err(QueryError::NotTreeSitter(_))
    ));
    let q = Query::new(Language::Rust, "(identifier) @x").unwrap();
    let py = parse(Path::new("a.py"), "x = 1\n").unwrap();
    assert!(matches!(
        q.run(&py, "x = 1\n"),
        Err(QueryError::WrongLanguage { .. })
    ));
}
