//! Dead code and public API read from declarations in every language the semantic model knows:
//! what nothing mentions is found, and what a framework may wire by an annotation or a conventional
//! name is left alone.
#![allow(clippy::expect_used, clippy::unwrap_used)] // a failed expect is a failed test

use debuggatha_engine::analysis::{
    DeadCodeKind, Workspace, build_declared_public_api, detect_declared_dead_code, infer_boundaries,
};

fn unreferenced(files: &[(&str, &str)]) -> Vec<String> {
    let ws = Workspace::from_files(files.iter().map(|(p, c)| (*p, *c)));
    let mut found: Vec<String> = detect_declared_dead_code(&ws)
        .into_iter()
        .filter(|f| f.kind == DeadCodeKind::Unreferenced)
        .map(|f| f.symbol)
        .collect();
    found.sort();
    found
}

#[test]
fn a_hidden_declaration_nothing_mentions_is_dead_in_each_language() {
    let cases: [(&str, &str); 9] = [
        (
            "a.go",
            "package a\nfunc live() {}\nfunc dead() {}\nfunc Entry() { live() }\n",
        ),
        (
            "a.py",
            "def _live(): pass\ndef _dead(): pass\ndef entry():\n    _live()\n",
        ),
        (
            "A.java",
            "public class A {\n private void live() {}\n private void dead() {}\n public void entry() { live(); }\n}\n",
        ),
        (
            "a.kt",
            "private fun live() {}\nprivate fun dead() {}\nfun entry() { live() }\n",
        ),
        (
            "A.cs",
            "public class A {\n private void Live() {}\n private void Dead() {}\n public void Entry() { Live(); }\n}\n",
        ),
        (
            "a.php",
            "<?php\nclass A {\n private function live() {}\n private function dead() {}\n public function entry() { $this->live(); }\n}\n",
        ),
        (
            "a.dart",
            "void _live() {}\nvoid _dead() {}\nvoid entry() { _live(); }\n",
        ),
        (
            "a.ex",
            "defmodule A do\n  defp live, do: 1\n  defp dead, do: 2\n  def entry, do: live()\nend\n",
        ),
        ("a.rb", "class A\n  def entry; end\nend\n"),
    ];
    for (file, source) in cases {
        let found = unreferenced(&[(file, source)]);
        if file == "a.rb" {
            assert_eq!(found, Vec::<String>::new(), "{file}");
        } else {
            assert!(
                found.contains(&"dead".to_owned())
                    || found.contains(&"_dead".to_owned())
                    || found.contains(&"Dead".to_owned()),
                "{file}: {found:?}"
            );
            assert!(
                !found.iter().any(|n| n.to_lowercase().contains("live")),
                "{file}: {found:?}"
            );
        }
    }
}

#[test]
fn a_name_another_file_of_the_directory_mentions_is_alive() {
    assert_eq!(
        unreferenced(&[
            ("p/a.go", "package p\nfunc helper() {}\n"),
            ("p/b.go", "package p\nfunc Use() { helper() }\n"),
        ]),
        Vec::<String>::new()
    );
    assert_eq!(
        unreferenced(&[
            ("p/a.go", "package p\nfunc helper() {}\n"),
            ("q/b.go", "package q\nfunc Use() { helper() }\n"),
        ]),
        vec!["helper".to_owned()]
    );
}

#[test]
fn a_decorated_or_conventional_declaration_may_be_wired_by_a_framework() {
    assert_eq!(
        unreferenced(&[(
            "a.py",
            "@app.route('/')\ndef _index(): pass\ndef _setup(): pass\ndef __repr__(): pass\n"
        )]),
        vec!["_setup".to_owned()]
    );
    assert_eq!(
        unreferenced(&[(
            "A.java",
            "public class A {\n @Bean private void wired() {}\n public static void main(String[] a) {}\n}\n"
        )]),
        Vec::<String>::new()
    );
}

#[test]
fn a_public_declaration_no_other_file_mentions_is_reported_at_low_confidence() {
    let ws = Workspace::from_files([
        ("a.go", "package a\nfunc Used() {}\nfunc Unused() {}\n"),
        ("b.go", "package a\nfunc Other() { Used() }\n"),
    ]);
    let mut found: Vec<_> = detect_declared_dead_code(&ws)
        .into_iter()
        .filter(|f| f.kind == DeadCodeKind::ExportedButUnused)
        .map(|f| f.symbol)
        .collect();
    found.sort();
    assert_eq!(found, vec!["Other".to_owned(), "Unused".to_owned()]);
}

#[test]
fn the_public_surface_lists_public_declarations_and_leaves_internal_ones_out() {
    let ws = Workspace::from_files([
        ("go.mod", "module example.com/m\n"),
        ("m.go", "package m\nfunc Serve() {}\nfunc serve() {}\n"),
        ("internal/x/x.go", "package x\nfunc Hidden() {}\n"),
        ("m_test.go", "package m\nfunc TestServe() {}\n"),
    ]);
    let found = infer_boundaries(&ws);
    let surfaces = build_declared_public_api(&ws, &found.list);
    let names: Vec<&str> = surfaces
        .iter()
        .flat_map(|s| s.exported.iter().map(|e| e.name.as_str()))
        .collect();
    assert_eq!(names, vec!["Serve"]);
}

#[test]
fn a_repository_that_calls_by_a_computed_name_makes_the_finding_a_guess() {
    use debuggatha_engine::analysis::Confidence;
    let plain = Workspace::from_files([(
        "a.php",
        "<?php\nclass A {\n private function dead() {}\n public function run() {}\n}\n",
    )]);
    let computed = Workspace::from_files([(
        "a.php",
        "<?php\nclass A {\n private function dead() {}\n public function run($m) { return $this->$m(); }\n}\n",
    )]);
    let confidence = |ws: &Workspace| {
        detect_declared_dead_code(ws)
            .into_iter()
            .find(|f| f.symbol == "dead")
            .map(|f| f.confidence)
    };
    assert_eq!(confidence(&plain), Some(Confidence::Medium));
    assert_eq!(confidence(&computed), Some(Confidence::Low));
}

#[test]
fn swift_declarations_have_the_visibility_the_language_gives() {
    let ws = Workspace::from_files([
        (
            "Package.swift",
            "// swift-tools-version:6.0\nimport PackageDescription\nlet package = Package(name: \"Ios\")\n",
        ),
        (
            "Sources/Ios/A.swift",
            "public struct Shown {}\nstruct Internal {}\nprivate func hidden() {}\nopen class Base {}\n",
        ),
    ]);
    let found = infer_boundaries(&ws);
    let mut names: Vec<String> = build_declared_public_api(&ws, &found.list)
        .into_iter()
        .flat_map(|s| s.exported.into_iter().map(|e| e.name))
        .collect();
    names.sort();
    assert_eq!(names, vec!["Base".to_owned(), "Shown".to_owned()]);
    assert_eq!(
        unreferenced(&[(
            "Sources/Ios/A.swift",
            "private func hidden() {}\npublic func entry() {}\n"
        )]),
        vec!["hidden".to_owned()]
    );
}
