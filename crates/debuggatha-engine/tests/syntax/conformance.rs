//! The Rust syntax layer against the TypeScript line lexer (`scripts/parity/gen-syntax-fixtures.ts` at tag `ts-engine-final`).
//! Cases marked "same" must produce the identical masked text; cases marked "differs" are the ones
//! where the lexer is wrong and the parse is right, and the expected Rust text is stated here.
#![allow(clippy::expect_used, clippy::unwrap_used)] // a failed expect is a failed test

use std::path::Path;

use debuggatha_engine::syntax::{ParseStatus, mask_source, parse};
use serde_json::Value;

fn fixture() -> Value {
    let path = format!(
        "{}/tests/fixtures/syntax-conformance.json",
        env!("CARGO_MANIFEST_DIR")
    );
    serde_json::from_str(&std::fs::read_to_string(path).expect("fixture")).expect("JSON")
}

fn masked(file: &str, source: &str) -> String {
    let syntax = parse(Path::new(file), source).expect("a supported language");
    assert_eq!(syntax.status, ParseStatus::Parsed, "{file}: {source}");
    mask_source(source, &syntax.facts.expect("facts"))
}

#[test]
fn where_the_lexer_is_right_the_parse_agrees() {
    for case in fixture()["cases"].as_array().unwrap() {
        if case["expect"] != "same" {
            continue;
        }
        let (file, source) = (
            case["file"].as_str().unwrap(),
            case["source"].as_str().unwrap(),
        );
        assert_eq!(
            masked(file, source),
            case["tsMasked"].as_str().unwrap(),
            "{}",
            case["name"]
        );
    }
}

#[test]
fn where_the_lexer_is_wrong_the_parse_is_right() {
    let want = [
        ("template-with-expr", "const s = `  ${process.env.X}  `;\n"),
        (
            "jsx-text-apostrophe",
            "export const A = () => <p>don't process.env.X</p>;\nconst k = process.env.K;\n",
        ),
        (
            "nested-template",
            "const s = `  ${`  ${process.env.X}`}  `;\n",
        ),
    ];
    let fx = fixture();
    for (name, expected) in want {
        let case = fx["cases"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["name"] == name)
            .unwrap();
        assert_eq!(case["expect"], "differs", "{name}");
        let got = masked(
            case["file"].as_str().unwrap(),
            case["source"].as_str().unwrap(),
        );
        assert_eq!(got, expected, "{name}");
        assert_ne!(
            got,
            case["tsMasked"].as_str().unwrap(),
            "{name} is listed as a divergence but now identical"
        );
    }
}
