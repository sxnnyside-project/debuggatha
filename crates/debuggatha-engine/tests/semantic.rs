//! The semantic model gives the same relations in every language: the call a reviewer cares about is
//! found with the loop it sits in, the function around it and whether it is test code.
#![allow(clippy::expect_used, clippy::unwrap_used)] // a failed expect is a failed test

use std::path::Path;

use debuggatha_engine::semantic::{Model, compile_errors, model_of};
use debuggatha_engine::syntax::parse;

fn model(file: &str, source: &str) -> Model {
    let syntax = parse(Path::new(file), source).expect("a supported language");
    model_of(&syntax, source).expect("an adapter for the language")
}

fn depth(model: &Model, callee: &str) -> Option<u32> {
    model
        .calls
        .iter()
        .find(|c| c.callee == callee)
        .map(|c| c.loop_depth)
}

#[test]
fn every_adapter_query_compiles() {
    assert_eq!(compile_errors(), Vec::<String>::new());
}

#[test]
fn a_call_in_a_loop_is_found_the_same_way_in_every_language() {
    // (file, source, callee inside a loop, callee outside any loop)
    let cases = [
        (
            "a.rs",
            "fn f(x: Vec<u8>) { for i in x { run(i); } done(); }",
            "run",
            "done",
        ),
        (
            "a.go",
            "package a\nfunc f(x []int) { for _, i := range x { run(i) }\n done() }",
            "run",
            "done",
        ),
        (
            "a.py",
            "def f(x):\n    for i in x:\n        run(i)\n    done()\n",
            "run",
            "done",
        ),
        (
            "a.kt",
            "fun f(x: List<Int>) { for (i in x) { run(i) }\n done() }",
            "run",
            "done",
        ),
        (
            "a.php",
            "<?php function f($x) { foreach ($x as $i) { run($i); } done(); }",
            "run",
            "done",
        ),
        (
            "A.java",
            "class A { void f(int[] x) { for (int i : x) { run(i); } done(); } }",
            "run",
            "done",
        ),
        (
            "a.cs",
            "class A { void F(int[] x) { foreach (var i in x) { Run(i); } Done(); } }",
            "Run",
            "Done",
        ),
        (
            "a.rb",
            "def f(x)\n  while x\n    run(x)\n  end\n  done(x)\nend\n",
            "run",
            "done",
        ),
        (
            "a.swift",
            "func f(x: [Int]) { for i in x { run(i) }\n done(1) }",
            "run",
            "done",
        ),
        (
            "a.dart",
            "void f(List x) { for (var i in x) { run(i); } done(); }",
            "run",
            "done",
        ),
        (
            "a.ex",
            "defmodule A do\n  def f(x) do\n    for i <- x, do: run(i)\n    done(x)\n  end\nend\n",
            "run",
            "done",
        ),
        (
            "a.cpp",
            "void f(int* x) { for (int i = 0; i < 3; i++) { run(i); } done(); }",
            "run",
            "done",
        ),
        (
            "a.gd",
            "func f(x):\n\tfor i in x:\n\t\trun(i)\n\tdone()\n",
            "run",
            "done",
        ),
    ];
    for (file, source, inside, outside) in cases {
        let m = model(file, source);
        assert_eq!(depth(&m, inside), Some(1), "{file}: {inside}");
        assert_eq!(depth(&m, outside), Some(0), "{file}: {outside}");
    }
}

#[test]
fn a_qualified_callee_reads_the_same_in_every_notation() {
    let cases = [
        (
            "a.rs",
            "fn f() { std::process::exit(1); }",
            "std.process.exit",
        ),
        (
            "a.php",
            "<?php function f() { \\Foo\\bar(); $o->run(); }",
            "Foo.bar",
        ),
        ("a.php", "<?php function f() { $o->run(); }", "$o.run"),
        (
            "A.java",
            "class A { void f() { System.exit(1); } }",
            "System.exit",
        ),
        (
            "a.ex",
            "defmodule A do\n  def f, do: System.halt(1)\nend\n",
            "System.halt",
        ),
        ("a.cpp", "void f() { std::abort(); }", "std.abort"),
    ];
    for (file, source, callee) in cases {
        let m = model(file, source);
        assert!(
            m.calls.iter().any(|c| c.callee == callee),
            "{file}: {callee} in {:?}",
            m.calls.iter().map(|c| &c.callee).collect::<Vec<_>>()
        );
    }
}

#[test]
fn test_code_is_marked_where_the_language_marks_it() {
    let cases = [
        (
            "a.rs",
            "#[test]\nfn t() { run(); }\nfn u() { run2(); }",
            "run",
            "run2",
        ),
        (
            "a.py",
            "def test_a():\n    run()\ndef u():\n    run2()\n",
            "run",
            "run2",
        ),
        (
            "a.kt",
            "@Test fun t() { run() }\nfun u() { run2() }",
            "run",
            "run2",
        ),
        (
            "A.java",
            "class A { @Test void t() { run(); } void u() { run2(); } }",
            "run",
            "run2",
        ),
    ];
    for (file, source, tested, shipped) in cases {
        let m = model(file, source);
        let flag = |name: &str| m.calls.iter().find(|c| c.callee == name).map(|c| c.in_test);
        assert_eq!(flag(tested), Some(true), "{file}: {tested}");
        assert_eq!(flag(shipped), Some(false), "{file}: {shipped}");
    }
}

#[test]
fn a_call_knows_its_function_and_a_closure_resets_the_loops() {
    let m = model(
        "a.rs",
        "fn f(x: Vec<u8>) { for i in x { let g = || run(i); g(); } }",
    );
    let run = m.calls.iter().find(|c| c.callee == "run").unwrap();
    let g = m.calls.iter().find(|c| c.callee == "g").unwrap();
    assert_eq!(run.loop_depth, 0);
    assert_eq!(g.loop_depth, 1);
    assert_ne!(run.function, g.function);
}

#[test]
fn a_call_in_a_broken_region_is_not_concluded_about() {
    let m = model("a.py", "def f(:\n    run(\n");
    assert_eq!(m.calls.len(), 0);
}

#[test]
fn javascript_fills_the_same_relations_as_the_other_languages() {
    let m = model(
        "a.ts",
        "import { q } from './db';\nexport function f(ids: number[]) {\n  for (const id of ids) { q(id); }\n  done();\n  ids.forEach((x) => q(x));\n}\n",
    );
    let depth = |callee: &str| {
        m.calls
            .iter()
            .filter(|c| c.callee == callee)
            .map(|c| c.loop_depth)
            .collect::<Vec<_>>()
    };
    assert_eq!(depth("q"), vec![1, 1]);
    assert_eq!(depth("done"), vec![0]);
    assert_eq!(
        m.imports
            .iter()
            .map(|i| i.specifier.as_str())
            .collect::<Vec<_>>(),
        vec!["./db"]
    );
    let f = m.declarations.iter().find(|d| d.name == "f").unwrap();
    assert_eq!(
        f.visibility,
        debuggatha_engine::semantic::Visibility::Public
    );
}
