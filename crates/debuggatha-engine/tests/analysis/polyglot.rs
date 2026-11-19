//! One analysis over a workspace of several languages: every module gets the reader of its own
//! ecosystem, a language nothing reads is listed, and a change reaches the modules that depend on it.
#![allow(clippy::expect_used, clippy::unwrap_used)] // a failed expect is a failed test

use debuggatha_engine::analysis::{DeadCodeKind, Workspace, analyze};

fn workspace() -> Workspace {
    Workspace::from_files([
        (
            "package.json",
            r#"{"name":"root","private":true,"workspaces":["packages/*"]}"#,
        ),
        (
            "packages/web/package.json",
            r#"{"name":"web","main":"src/index.js"}"#,
        ),
        (
            "packages/web/src/index.js",
            "export function shown() { return 1; }\n",
        ),
        ("Cargo.toml", "[workspace]\nmembers = [\"core\"]\n"),
        (
            "core/Cargo.toml",
            "[package]\nname = \"core\"\n[lib]\npath = \"src/lib.rs\"\n",
        ),
        ("core/src/lib.rs", "pub fn shown() {}\n"),
        ("svc/pyproject.toml", "[project]\nname = \"svc\"\n"),
        (
            "svc/src/svc/m.py",
            "def _never_called():\n    return 1\n\ndef used():\n    return 2\n\nused()\n",
        ),
        ("api/go.mod", "module example.com/api\n"),
        ("api/main.go", "package main\nfunc main() {}\n"),
        (
            "ios/Package.swift",
            "// swift-tools-version:6.0\nimport PackageDescription\nlet package = Package(name: \"Ios\")\n",
        ),
        (
            "ios/Sources/Ios/A.swift",
            "public struct Shown {}\nprivate func hidden() {}\n",
        ),
        ("native/CMakeLists.txt", "project(native)\n"),
        ("native/a.cpp", "int main() { return 0; }\n"),
    ])
}

#[test]
fn each_module_is_read_by_the_reader_of_its_ecosystem() {
    let analysis = analyze(&workspace());
    let language_of = |module: &str| {
        analysis
            .public_api
            .iter()
            .find(|s| s.module_id == module)
            .map(|s| (s.language, s.covered))
    };
    assert_eq!(language_of("packages/web"), Some(("javascript", true)));
    assert_eq!(language_of("core"), Some(("rust", true)));
    assert_eq!(language_of("svc"), Some(("python", true)));
    assert_eq!(language_of("api"), Some(("go", true)));
    assert_eq!(language_of("ios"), Some(("swift", true)));
    let modules: Vec<_> = analysis.public_api.iter().map(|s| &s.module_id).collect();
    let mut unique = modules.clone();
    unique.dedup();
    assert_eq!(modules.len(), unique.len(), "a module has one surface");
}

#[test]
fn dead_code_is_found_in_a_language_that_is_not_javascript() {
    let analysis = analyze(&workspace());
    assert!(
        analysis
            .dead_code
            .iter()
            .any(|d| d.symbol == "_never_called" && d.kind == DeadCodeKind::Unreferenced),
        "{:?}",
        analysis.dead_code
    );
    assert!(
        !analysis
            .dead_code
            .iter()
            .any(|d| d.symbol == "used" && d.kind == DeadCodeKind::Unreferenced)
    );
}

#[test]
fn a_language_no_stage_reads_is_listed_and_not_called_clean() {
    let analysis = analyze(&workspace());
    let listed: Vec<_> = analysis
        .not_covered
        .iter()
        .map(|n| (n.language, n.files))
        .collect();
    assert_eq!(listed, vec![("cpp", 1), ("rust", 1)]);
}

#[test]
fn a_change_reaches_the_module_that_owns_the_file() {
    let analysis = analyze(&workspace());
    let impact = analysis.change_impact(&["svc/src/svc/m.py".to_owned()]);
    assert_eq!(impact.affected_modules, vec!["svc".to_owned()]);
}

#[test]
fn a_diff_names_the_exports_on_the_lines_it_touches() {
    use debuggatha_engine::diff::parse_unified_diff;
    let analysis = analyze(&workspace());
    let touches_export = "diff --git a/packages/web/src/index.js b/packages/web/src/index.js
--- a/packages/web/src/index.js
+++ b/packages/web/src/index.js
@@ -1 +1 @@
-export function shown() { return 0; }
+export function shown() { return 1; }
";
    let impact = analysis.change_impact_of_diff(&parse_unified_diff(touches_export));
    let touched: Vec<_> = impact
        .exports_on_changed_lines
        .iter()
        .map(|s| s.name.as_str())
        .collect();
    assert_eq!(touched, ["shown"]);

    let elsewhere = "diff --git a/packages/web/src/index.js b/packages/web/src/index.js
--- a/packages/web/src/index.js
+++ b/packages/web/src/index.js
@@ -1 +1,2 @@
 export function shown() { return 1; }
+const unrelated = 2;
";
    let impact = analysis.change_impact_of_diff(&parse_unified_diff(elsewhere));
    assert_eq!(impact.exports_on_changed_lines.len(), 0);
    assert_eq!(
        impact.affected_public_api.len(),
        1,
        "the file still holds an export"
    );
}

#[test]
fn a_removed_file_lowers_the_confidence_and_is_named() {
    use debuggatha_engine::analysis::ImpactConfidence;
    use debuggatha_engine::diff::parse_unified_diff;
    let analysis = analyze(&workspace());
    let removed = "diff --git a/svc/src/svc/m.py b/svc/src/svc/m.py
deleted file mode 100644
--- a/svc/src/svc/m.py
+++ /dev/null
@@ -1 +0,0 @@
-def used(): pass
";
    let impact = analysis.change_impact_of_diff(&parse_unified_diff(removed));
    assert_eq!(impact.deleted_files, ["svc/src/svc/m.py"]);
    assert_ne!(impact.confidence, ImpactConfidence::High);
    assert!(impact.rationale.contains("removed file"));
}
