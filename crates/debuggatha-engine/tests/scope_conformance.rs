//! The scan scope predicates against the TypeScript (`scripts/parity/gen-scope-fixtures.ts`).
#![allow(clippy::expect_used, clippy::unwrap_used)] // a failed expect is a failed test

use debuggatha_engine::scan_scope::{is_generated_content, is_ignored_path, is_test_path};
use serde_json::Value;

fn fixture() -> Value {
    let path = format!(
        "{}/tests/fixtures/scope-conformance.json",
        env!("CARGO_MANIFEST_DIR")
    );
    serde_json::from_str(&std::fs::read_to_string(path).expect("fixture")).expect("JSON")
}

#[test]
fn paths_are_ignored_or_tests_exactly_as_in_typescript() {
    for case in fixture()["paths"].as_array().unwrap() {
        let path = case["path"].as_str().unwrap();
        assert_eq!(
            is_ignored_path(None, path),
            case["ignored"],
            "ignored: {path:?}"
        );
        assert_eq!(is_test_path(path), case["test"], "test: {path:?}");
    }
}

#[test]
fn generated_content_is_told_apart_as_in_typescript() {
    for case in fixture()["contents"].as_array().unwrap() {
        assert_eq!(
            is_generated_content(case["content"].as_str().unwrap()),
            case["generated"],
            "{}",
            case["name"]
        );
    }
}

#[test]
fn bin_and_obj_are_build_output_only_beside_a_build_tool() {
    let root = std::env::temp_dir().join(format!("dbg-scope-{}", std::process::id()));
    for case in fixture()["markers"].as_array().unwrap() {
        let marker = case["marker"].as_str();
        let dir = root.join(marker.unwrap_or("none"));
        std::fs::create_dir_all(&dir).unwrap();
        if let Some(m) = marker {
            std::fs::write(dir.join(m), "").unwrap();
        }
        let path = case["path"].as_str().unwrap();
        assert_eq!(
            is_ignored_path(Some(&dir), path),
            case["ignored"],
            "{marker:?} {path}"
        );
    }
    std::fs::remove_dir_all(root).ok();
}

#[test]
fn kotlin_multiplatform_test_source_sets_are_tests() {
    assert!(is_test_path("okhttp/src/jvmTest/kotlin/A.kt"));
    assert!(is_test_path("lib/src/commonTest/kotlin/A.kt"));
    assert!(!is_test_path("lib/src/jvmMain/kotlin/A.kt"));
}

#[test]
fn a_test_class_is_named_by_capital_letters() {
    assert!(is_test_path("src/FooTest.java"));
    assert!(is_test_path("src/FooTests.cs"));
    assert!(is_test_path("src/test_models.py"));
    assert!(is_test_path("spec/models/user_spec.rb"));
    assert!(!is_test_path("src/Commit.java"));
    assert!(!is_test_path("src/Split.kt"));
}
