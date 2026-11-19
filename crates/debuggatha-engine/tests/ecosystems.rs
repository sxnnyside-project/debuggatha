//! The graph reads modules and imports of every ecosystem the descriptors name, not npm and Cargo
//! only: two modules, one importing the other, give an edge in each language, and an import of a
//! dependency gives none.
#![allow(clippy::expect_used, clippy::unwrap_used)] // a failed expect is a failed test

use debuggatha_engine::analysis::{Workspace, build_graph, infer_boundaries};

fn edges(files: &[(&str, &str)]) -> Vec<(String, String)> {
    let ws = Workspace::from_files(files.iter().map(|(p, c)| (*p, *c)));
    let found = infer_boundaries(&ws);
    build_graph(&ws, &found)
        .edges
        .into_iter()
        .map(|e| (e.from, e.to))
        .collect()
}

fn a_to_b() -> Vec<(String, String)> {
    vec![("a".to_owned(), "b".to_owned())]
}

#[test]
fn go_modules_import_each_other_by_module_path() {
    let found = edges(&[
        ("a/go.mod", "module example.com/a\n\ngo 1.22\n"),
        (
            "a/main.go",
            "package main\nimport (\n\t\"fmt\"\n\t\"example.com/b/util\"\n)\nfunc main() { fmt.Println(util.X) }\n",
        ),
        ("b/go.mod", "module example.com/b\n"),
        ("b/util/u.go", "package util\nvar X = 1\n"),
    ]);
    assert_eq!(found, a_to_b());
}

#[test]
fn python_packages_import_each_other_under_their_source_root() {
    let found = edges(&[
        ("a/pyproject.toml", "[project]\nname = \"a\"\n"),
        ("a/src/a/x.py", "import requests\nfrom b.y import z\n"),
        ("b/pyproject.toml", "[project]\nname = \"b\"\n"),
        ("b/src/b/y.py", "z = 1\n"),
    ]);
    assert_eq!(found, a_to_b());
}

#[test]
fn dart_packages_import_each_other_by_package_uri() {
    let found = edges(&[
        ("a/pubspec.yaml", "name: a\nversion: 1.0.0\n"),
        (
            "a/lib/a.dart",
            "import 'package:b/b.dart';\nimport 'dart:io';\n",
        ),
        ("b/pubspec.yaml", "name: b\n"),
        ("b/lib/b.dart", "int x = 1;\n"),
    ]);
    assert_eq!(found, a_to_b());
}

#[test]
fn composer_packages_import_each_other_by_namespace() {
    let found = edges(&[
        (
            "a/composer.json",
            "{\"name\":\"v/a\",\"autoload\":{\"psr-4\":{\"A\\\\\":\"src/\"}}}",
        ),
        (
            "a/src/X.php",
            "<?php\nnamespace A;\nuse B\\Util;\nuse Illuminate\\Support\\Str;\n",
        ),
        (
            "b/composer.json",
            "{\"name\":\"v/b\",\"autoload\":{\"psr-4\":{\"B\\\\\":\"src/\"}}}",
        ),
        ("b/src/Util.php", "<?php\nnamespace B;\nclass Util {}\n"),
    ]);
    assert_eq!(found, a_to_b());
}

#[test]
fn mix_apps_import_each_other_by_module_name() {
    let mix = |app: &str| {
        format!(
            "defmodule M.MixProject do\n  def project, do: [app: :{app}, version: \"0.1.0\"]\nend\n"
        )
    };
    let found = edges(&[
        ("a/mix.exs", &mix("a")),
        (
            "a/lib/a/x.ex",
            "defmodule A.X do\n  alias B.Util\n  import Enum\nend\n",
        ),
        ("b/mix.exs", &mix("b")),
        ("b/lib/b/util.ex", "defmodule B.Util do\nend\n"),
    ]);
    assert_eq!(found, a_to_b());
}

#[test]
fn maven_modules_import_each_other_by_package() {
    let pom = |id: &str| format!("<project><artifactId>{id}</artifactId></project>");
    let found = edges(&[
        ("a/pom.xml", &pom("a")),
        (
            "a/src/main/java/com/a/X.java",
            "package com.a;\nimport com.b.Util;\nimport java.util.List;\nclass X {}\n",
        ),
        ("b/pom.xml", &pom("b")),
        (
            "b/src/main/java/com/b/Util.java",
            "package com.b;\nclass Util {}\n",
        ),
    ]);
    assert_eq!(found, a_to_b());
}

#[test]
fn dotnet_projects_import_each_other_by_namespace() {
    let proj = |ns: &str| {
        format!(
            "<Project><PropertyGroup><RootNamespace>{ns}</RootNamespace></PropertyGroup></Project>"
        )
    };
    let found = edges(&[
        ("a/A.csproj", &proj("A")),
        (
            "a/X.cs",
            "using B.Core;\nusing System;\nnamespace A { class X {} }\n",
        ),
        ("b/B.csproj", &proj("B")),
        ("b/Y.cs", "namespace B.Core { class Y {} }\n"),
    ]);
    assert_eq!(found, a_to_b());
}

#[test]
fn a_python_import_of_a_missing_module_of_the_repository_is_reported() {
    let ws = Workspace::from_files([
        ("a/pyproject.toml", "[project]\nname = \"a\"\n"),
        ("a/src/a/x.py", "from a.missing import q\nimport numpy\n"),
        ("a/src/a/y.py", "q = 1\n"),
    ]);
    let found = infer_boundaries(&ws);
    let graph = build_graph(&ws, &found);
    let missing: Vec<_> = graph
        .unresolved
        .iter()
        .filter_map(|u| u.specifier.as_deref())
        .collect();
    assert_eq!(missing, vec!["a.missing"]);
}

fn declared(files: &[(&str, &str)]) -> Vec<(String, String)> {
    let ws = Workspace::from_files(files.iter().map(|(p, c)| (*p, *c)));
    let found = infer_boundaries(&ws);
    build_graph(&ws, &found)
        .edges
        .into_iter()
        .filter(|e| e.kind == debuggatha_engine::analysis::EdgeKind::Declared)
        .map(|e| (e.from, e.to))
        .collect()
}

#[test]
fn manifests_declare_dependencies_on_modules_of_the_repository() {
    let cases: Vec<Vec<(&str, &str)>> = vec![
        vec![
            (
                "a/go.mod",
                "module example.com/a\nrequire (\n\texample.com/b v1.0.0\n\tgithub.com/x/y v2.0.0 // indirect\n)\n",
            ),
            ("a/a.go", "package a\n"),
            ("b/go.mod", "module example.com/b\n"),
            ("b/b.go", "package b\n"),
        ],
        vec![
            (
                "a/composer.json",
                "{\"name\":\"v/a\",\"require\":{\"php\":\"^8\",\"v/b\":\"^1\"}}",
            ),
            ("a/src/A.php", "<?php\n"),
            ("b/composer.json", "{\"name\":\"v/b\"}"),
            ("b/src/B.php", "<?php\n"),
        ],
        vec![
            (
                "a/pubspec.yaml",
                "name: a\ndependencies:\n  flutter:\n    sdk: flutter\n  b:\n    path: ../b\n",
            ),
            ("a/lib/a.dart", "int a = 1;\n"),
            ("b/pubspec.yaml", "name: b\n"),
            ("b/lib/b.dart", "int b = 1;\n"),
        ],
        vec![
            (
                "a/pyproject.toml",
                "[project]\nname = \"a\"\ndependencies = [\"requests>=2\", \"B-lib[extra]>=1; python_version<'4'\"]\n",
            ),
            ("a/src/a/x.py", "x = 1\n"),
            ("b/pyproject.toml", "[project]\nname = \"b_lib\"\n"),
            ("b/src/b_lib/y.py", "y = 1\n"),
        ],
        vec![
            (
                "a/A.csproj",
                "<Project><ItemGroup><ProjectReference Include=\"..\\b\\B.csproj\" /></ItemGroup></Project>",
            ),
            ("a/X.cs", "class X {}\n"),
            ("b/B.csproj", "<Project></Project>"),
            ("b/Y.cs", "class Y {}\n"),
        ],
        vec![
            (
                "a/pom.xml",
                "<project><artifactId>a</artifactId><dependencies><dependency><groupId>g</groupId><artifactId>b</artifactId></dependency></dependencies></project>",
            ),
            (
                "a/src/main/java/com/a/X.java",
                "package com.a;\nclass X {}\n",
            ),
            ("b/pom.xml", "<project><artifactId>b</artifactId></project>"),
            (
                "b/src/main/java/com/b/Y.java",
                "package com.b;\nclass Y {}\n",
            ),
        ],
    ];
    for (n, files) in cases.iter().enumerate() {
        assert_eq!(declared(files), a_to_b(), "case {n}");
    }
}

#[test]
fn the_directories_of_a_single_module_are_its_modules() {
    let go = edges(&[
        ("go.mod", "module example.com/m\n"),
        ("main.go", "package main\n"),
        (
            "internal/a/a.go",
            "package a\nimport \"example.com/m/internal/b\"\nvar X = b.Y\n",
        ),
        ("internal/b/b.go", "package b\nvar Y = 1\n"),
    ]);
    assert_eq!(go, vec![("internal/a".to_owned(), "internal/b".to_owned())]);
    let py = edges(&[
        ("pyproject.toml", "[project]\nname = \"m\"\n"),
        ("app/x.py", "from core.y import z\n"),
        ("core/y.py", "z = 1\n"),
    ]);
    assert_eq!(py, vec![("app".to_owned(), "core".to_owned())]);
}

#[test]
fn mix_and_gradle_declare_dependencies_on_sibling_projects() {
    let mix = |app: &str, deps: &str| {
        format!(
            "defmodule M.MixProject do\n  def project, do: [app: :{app}]\n  defp deps do\n    [{deps}]\n  end\nend\n"
        )
    };
    let found = edges(&[
        (
            "a/mix.exs",
            &mix("a", "{:b, path: \"../b\"}, {:phoenix, \"~> 1.7\"}"),
        ),
        ("a/lib/a.ex", "defmodule A do\nend\n"),
        ("b/mix.exs", &mix("b", "")),
        ("b/lib/b.ex", "defmodule B do\nend\n"),
        (
            "app/build.gradle.kts",
            "dependencies {\n    implementation(project(\":core\"))\n}\n",
        ),
        ("app/src/main/kotlin/App.kt", "package app\n"),
        ("core/build.gradle.kts", "plugins {}\n"),
        ("core/src/main/kotlin/Core.kt", "package core\n"),
    ]);
    assert_eq!(
        found,
        vec![
            ("a".to_owned(), "b".to_owned()),
            ("app".to_owned(), "core".to_owned())
        ]
    );
}

#[test]
fn swift_packages_import_each_other_by_module_name() {
    let manifest = |name: &str, deps: &str| {
        format!(
            "// swift-tools-version:6.0\nimport PackageDescription\nlet package = Package(\n    name: \"{name}\",\n    dependencies: [{deps}]\n)\n"
        )
    };
    let found = edges(&[
        (
            "a/Package.swift",
            &manifest("A", ".package(path: \"../b\")"),
        ),
        ("a/Sources/A/A.swift", "import B\nimport Foundation\n"),
        ("b/Package.swift", &manifest("B", "")),
        ("b/Sources/B/B.swift", "public struct B {}\n"),
    ]);
    // One edge from the declared dependency and one from the import: both are evidence.
    let mut found = found;
    found.dedup();
    assert_eq!(found, a_to_b());
}

#[test]
fn every_extension_an_ecosystem_names_is_analyzed() {
    use debuggatha_engine::analysis::ecosystem::descriptors;
    use debuggatha_engine::analysis::workspace::ANALYZED_EXTENSIONS;
    for (id, descriptor) in descriptors() {
        for extension in &descriptor.extensions {
            assert!(
                ANALYZED_EXTENSIONS.contains(&extension.as_str()),
                "{id} names .{extension}, which no stage reads"
            );
        }
    }
}

#[test]
fn an_import_of_the_package_at_the_root_is_an_edge_between_existing_modules() {
    use debuggatha_engine::analysis::{Workspace, build_graph, infer_boundaries};
    let ws = Workspace::from_files([
        (
            "Package.swift",
            "// swift-tools-version:6.0\nimport PackageDescription\nlet package = Package(name: \"App\")\n",
        ),
        ("Sources/App/A.swift", "import App\n"),
    ]);
    let graph = build_graph(&ws, &infer_boundaries(&ws));
    for edge in &graph.edges {
        assert!(graph.nodes.contains(&edge.from), "{edge:?}");
        assert!(graph.nodes.contains(&edge.to), "{edge:?}");
    }
}
