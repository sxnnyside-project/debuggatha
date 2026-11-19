//! Inline suppression against the TypeScript reference (`scripts/parity/gen-suppress-fixtures.ts`).
#![allow(clippy::expect_used, clippy::unwrap_used)] // a failed expect is a failed test

use std::path::Path;

use debuggatha_engine::syntax::{
    CommentLine, Directive, DirectiveScope, comment_lines, find_directive,
    inline_suppression_comment, parse, parse_directives,
};
use serde_json::Value;

fn fixture() -> Value {
    let path = format!(
        "{}/tests/fixtures/suppress-conformance.json",
        env!("CARGO_MANIFEST_DIR")
    );
    serde_json::from_str(&std::fs::read_to_string(path).expect("fixture")).expect("JSON")
}

fn lines(v: &Value) -> Vec<CommentLine> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|c| CommentLine {
            line: u32::try_from(c["line"].as_u64().unwrap()).unwrap(),
            text: c["text"].as_str().unwrap().to_owned(),
        })
        .collect()
}

fn expected(v: &Value) -> Vec<Directive> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|d| Directive {
            scope: match d["scope"].as_str().unwrap() {
                "line" => DirectiveScope::Line,
                "next-line" => DirectiveScope::NextLine,
                _ => DirectiveScope::File,
            },
            line: u32::try_from(d["line"].as_u64().unwrap()).unwrap(),
            rule_ids: d.get("ruleIds").map(|ids| {
                ids.as_array()
                    .unwrap()
                    .iter()
                    .map(|i| i.as_str().unwrap().to_owned())
                    .collect()
            }),
            reason: d.get("reason").map(|r| r.as_str().unwrap().to_owned()),
        })
        .collect()
}

#[test]
fn directives_and_coverage_match_typescript_for_the_same_comments() {
    for case in fixture()["cases"].as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        let directives = parse_directives(&lines(&case["comments"]));
        assert_eq!(
            directives,
            expected(&case["directives"]),
            "{name}: directives"
        );
        for probe in case["probes"].as_array().unwrap() {
            let (line, rule, want) = (
                u32::try_from(probe[0].as_u64().unwrap()).unwrap(),
                probe[1].as_str().unwrap(),
                probe[2].as_i64().unwrap(),
            );
            let got = find_directive(&directives, line, rule)
                .and_then(|d| directives.iter().position(|x| x == d))
                .map_or(-1, |i| i64::try_from(i).unwrap());
            assert_eq!(got, want, "{name}: line {line} rule {rule}");
        }
    }
}

#[test]
fn comments_from_the_syntax_tree_match_the_lexer_except_for_crlf() {
    for case in fixture()["cases"].as_array().unwrap() {
        let file = case["file"].as_str().unwrap();
        if Path::new(file).extension().is_none_or(|e| e != "ts") {
            continue; // no parser for the other languages yet
        }
        let source = case["source"].as_str().unwrap();
        let syntax = parse(Path::new(file), source).expect("supported");
        let got = comment_lines(source, &syntax.facts.expect("facts"));
        let mut want = lines(&case["comments"]);
        // The lexer keeps the `\r` of a CRLF file; the tree-based extraction does not.
        for c in &mut want {
            c.text = c.text.strip_suffix('\r').unwrap_or(&c.text).to_owned();
        }
        // The lexer sees `// ...` in a string-free source only; compare comment by comment.
        assert_eq!(got, want, "{}", case["name"]);
    }
}

#[test]
fn crlf_files_keep_their_directives_unlike_typescript() {
    for name in ["crlf-line", "crlf-block"] {
        let fx = fixture();
        let case = fx["cases"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["name"] == name)
            .unwrap();
        assert_eq!(case["note"], "ts-bug-crlf");
        assert!(
            expected(&case["directives"]).is_empty(),
            "{name}: TypeScript lost it"
        );
        let source = case["source"].as_str().unwrap();
        let syntax = parse(Path::new("a.ts"), source).expect("supported");
        let found = parse_directives(&comment_lines(source, &syntax.facts.unwrap()));
        assert_eq!(found.len(), 1, "{name}: Rust keeps the directive");
    }
}

#[test]
fn suppression_comment_syntax_matches_typescript() {
    for pair in fixture()["comments"].as_array().unwrap() {
        let file = pair[0].as_str().unwrap();
        assert_eq!(
            inline_suppression_comment(file, "no-eval", "because"),
            pair[1].as_str().unwrap(),
            "{file}"
        );
    }
}
