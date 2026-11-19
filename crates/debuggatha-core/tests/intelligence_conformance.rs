//! Differential tests against the TypeScript repository intelligence (ADR-0006, decision 8).
//!
//! `tests/fixtures/intelligence-conformance.json` is written by `scripts/parity/gen-intelligence-fixtures.ts` at tag `ts-engine-final`:
//! small repositories built on disk, scanned by the TypeScript code. Each case is rebuilt here in a
//! temporary directory and must give the same context (root path and build time are normalized).
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)] // a failed expect is a failed test

mod common;

use std::path::{Path, PathBuf};

use common::{CountingEnv, fixture, ignore_manifest_list, sort_unordered_evidence};
use debuggatha_core::repository_intelligence::{
    RepositoryContextCache, build_repository_context, summarize_capabilities,
};
use serde_json::{Value, json};

struct TempTree(PathBuf);

impl TempTree {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!("dg-intel-{}-{name}", std::process::id()));
        drop(std::fs::remove_dir_all(&path));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}

impl Drop for TempTree {
    fn drop(&mut self) {
        drop(std::fs::remove_dir_all(&self.0));
    }
}

/// Returns false when the case needs symbolic links and the platform cannot make them.
fn materialize(root: &Path, tree: &Value) -> bool {
    for (path, entry) in tree.as_object().unwrap() {
        let full = root.join(path);
        std::fs::create_dir_all(full.parent().unwrap()).unwrap();
        if let Some(text) = entry.as_str() {
            std::fs::write(&full, text).unwrap();
        } else {
            #[cfg(unix)]
            std::os::unix::fs::symlink(entry["symlink"].as_str().unwrap(), &full).unwrap();
            #[cfg(not(unix))]
            return false;
        }
    }
    true
}

/// The TypeScript engine copies whatever a manifest holds into `declaredVersions` and
/// `runtimeConstraints`. The contract says strings, so Rust reads numbers and booleans as text and
/// leaves out objects and arrays (docs/PARITY.md); the expectation gets the same rule applied.
fn apply_documented_divergence(context: &mut Value) {
    for map in ["declaredVersions", "runtimeConstraints"] {
        let entries = context["dependencies"][map].as_object_mut().unwrap();
        entries.retain(|_, v| matches!(v, Value::String(_) | Value::Number(_) | Value::Bool(_)));
        for v in entries.values_mut() {
            if !v.is_string() {
                *v = json!(v.to_string());
            }
        }
    }
}

fn normalized(context: &Value) -> Value {
    let mut context = context.clone();
    context["rootDir"] = json!("<root>");
    context["generatedAt"] = json!("<T>");
    context
}

#[test]
fn every_repository_gives_the_same_context_as_the_typescript_scanners() {
    let doc = fixture("intelligence-conformance.json");
    let cases = doc["cases"].as_array().unwrap();
    assert!(cases.len() >= 25);
    let (mut compared, mut errors) = (0, 0);
    for case in cases {
        let name = case["name"].as_str().unwrap();
        let tree = TempTree::new(name);
        if !materialize(&tree.0, &case["tree"]) {
            continue;
        }
        let env = CountingEnv::new("i");
        let result = build_repository_context(&tree.0, &env, None);
        let expected = &case["expected"];
        if expected.get("error").is_some() {
            assert!(
                result.is_err(),
                "{name}: the TypeScript scanners throw, Rust returned a context"
            );
            errors += 1;
            continue;
        }
        let context = result.unwrap_or_else(|e| panic!("{name}: {e}"));
        let mut got = normalized(&serde_json::to_value(&context).unwrap());
        sort_unordered_evidence(&mut got);
        let mut want = expected["context"].clone();
        apply_documented_divergence(&mut want);
        sort_unordered_evidence(&mut want);
        ignore_manifest_list(&mut got);
        ignore_manifest_list(&mut want);
        assert_eq!(got, want, "{name}: context");
        assert_eq!(
            summarize_capabilities(&context.capabilities),
            expected["summary"].as_str().unwrap(),
            "{name}: summary"
        );
        compared += 1;
    }
    assert!(compared >= 20, "only {compared} contexts were compared");
    assert_eq!(errors, 3);
}

#[test]
fn the_cache_serves_an_unchanged_repository_and_rebuilds_a_changed_one() {
    let tree = TempTree::new("cache");
    std::fs::write(
        tree.0.join("package.json"),
        r#"{ "dependencies": { "react": "18" } }"#,
    )
    .unwrap();
    std::fs::write(tree.0.join("README.md"), "# One\n").unwrap();
    let env = CountingEnv::new("c");
    let mut cache = RepositoryContextCache::default();

    let first = build_repository_context(&tree.0, &env, Some(&mut cache)).unwrap();
    let again = build_repository_context(&tree.0, &env, Some(&mut cache)).unwrap();
    assert_eq!(first, again);

    std::fs::write(tree.0.join("README.md"), "# Two, longer\n").unwrap();
    let after_edit = build_repository_context(&tree.0, &env, Some(&mut cache)).unwrap();
    assert_eq!(
        after_edit.documentation.sources[0].title.as_deref(),
        Some("Two, longer")
    );

    std::fs::write(tree.0.join("vite.config.ts"), "").unwrap();
    let after_new_file = build_repository_context(&tree.0, &env, Some(&mut cache)).unwrap();
    assert!(
        after_new_file
            .stack
            .build_systems
            .iter()
            .any(|s| s.value == "Vite")
    );
}
