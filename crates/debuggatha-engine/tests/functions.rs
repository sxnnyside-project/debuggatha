//! `require-await` and `no-loop-func` are specified by the cases below. Each
//! says what the tree proves (a positive) or the look-alike that must stay quiet (a negative), and
//! the expected hits are `[rule, line, column, confidence]`. Mutations of the detector must fail
//! at least one of them.
#![allow(clippy::expect_used, clippy::unwrap_used)]
// a failed expect is a failed test
// The tests are tables of cases: splitting a table only hides what it covers.
#![allow(clippy::too_many_lines)]

use std::path::Path;

use debuggatha_engine::detect::{DetectorInput, functions};
use debuggatha_engine::syntax::parse;

type Expected = &'static [(&'static str, u32, u32, &'static str)];

fn run(file: &str, source: &str) -> Vec<(String, u32, u32, String)> {
    let syntax = parse(Path::new(file), source).expect("a supported language");
    let input = DetectorInput::new(file, source, Some(&syntax));
    functions::detect(&input)
        .into_iter()
        .map(|(rule, h)| {
            (
                rule.to_owned(),
                h.line,
                h.columns.start,
                serde_json::to_value(h.confidence)
                    .unwrap()
                    .as_str()
                    .unwrap()
                    .to_owned(),
            )
        })
        .collect()
}

fn check(cases: &[(&str, &str, &str, Expected)]) {
    let mut bad = Vec::new();
    for (name, file, source, want) in cases {
        let got = run(file, source);
        let want: Vec<(String, u32, u32, String)> = want
            .iter()
            .map(|(r, l, c, k)| ((*r).to_owned(), *l, *c, (*k).to_owned()))
            .collect();
        if got != want {
            bad.push(format!("{name}: got {got:?}, want {want:?}"));
        }
    }
    assert!(bad.is_empty(), "\n{}", bad.join("\n"));
}

const RA: &str = "require-await";
const LF: &str = "no-loop-func";

#[test]
fn async_function_that_never_awaits() {
    check(&[
        (
            "declaration",
            "a.ts",
            "async function f() { return 1; }\n",
            &[(RA, 1, 1, "high")],
        ),
        (
            "exported",
            "a.ts",
            "export async function f() { return 1; }\n",
            &[(RA, 1, 8, "high")],
        ),
        (
            "arrow value",
            "a.ts",
            "const f = async () => 1;\n",
            &[(RA, 1, 11, "high")],
        ),
        (
            "arrow block",
            "a.ts",
            "const f = async () => { return 1; };\n",
            &[(RA, 1, 11, "high")],
        ),
        (
            "function expression",
            "a.js",
            "const f = async function () { return 1; };\n",
            &[(RA, 1, 11, "high")],
        ),
        (
            "class method",
            "a.ts",
            "class A {\n  async m() { return 1; }\n}\n",
            &[(RA, 2, 3, "high")],
        ),
        (
            "static class method",
            "a.ts",
            "class A {\n  static async m() { return 1; }\n}\n",
            &[(RA, 2, 3, "high")],
        ),
        (
            "returns a call",
            "a.ts",
            "async function f() { return g(); }\n",
            &[(RA, 1, 1, "medium")],
        ),
        (
            "returns a parenthesized call",
            "a.ts",
            "async function f() { return (g()); }\n",
            &[(RA, 1, 1, "medium")],
        ),
        (
            "returns new",
            "a.ts",
            "async function f() { return new Promise(r => r()); }\n",
            &[(RA, 1, 1, "medium")],
        ),
        (
            "arrow call value",
            "a.ts",
            "const f = async () => g();\n",
            &[(RA, 1, 11, "medium")],
        ),
        (
            "calls but does not return",
            "a.ts",
            "async function f() { g(); return 1; }\n",
            &[(RA, 1, 1, "high")],
        ),
        (
            "declaration inside a callback",
            "a.ts",
            "foo(() => {\n  async function g() { return 1; }\n  return g;\n});\n",
            &[(RA, 2, 3, "high")],
        ),
        (
            "export default",
            "a.ts",
            "export default async function () { return 1; }\n",
            &[(RA, 1, 16, "high")],
        ),
        (
            "class property without a parent",
            "a.ts",
            "class A { m = async () => 1; }\n",
            &[(RA, 1, 15, "high")],
        ),
        (
            "untyped declarator",
            "a.ts",
            "const f = async (req) => 1;\n",
            &[(RA, 1, 11, "high")],
        ),
        (
            "outer flagged, inner awaits",
            "a.ts",
            "async function f() { const g = async () => { await h(); }; return 1; }\n",
            &[(RA, 1, 1, "high")],
        ),
        (
            "declared another return type",
            "a.ts",
            "async function f(): number { return 1; }\n",
            &[(RA, 1, 1, "high")],
        ),
        (
            "declared a type named Promise inside a union",
            "a.ts",
            "async function f(): Promise<number> | null { return 1; }\n",
            &[(RA, 1, 1, "high")],
        ),
        (
            "decorated method: the report is on the method's line",
            "a.ts",
            "class A {\n  @Get('/x')\n  async m() { return 1; }\n}\n",
            &[(RA, 3, 3, "high")],
        ),
        (
            "two decorators and modifiers",
            "a.ts",
            "class A {\n  @Get('/x')\n  @Auth()\n  public static async m() { return 1; }\n}\n",
            &[(RA, 4, 3, "high")],
        ),
        (
            "decorator on the same line",
            "a.ts",
            "class A {\n  @Get('/x') async m() { return 1; }\n}\n",
            &[(RA, 2, 14, "high")],
        ),
        (
            "two on separate lines",
            "a.ts",
            "async function a() { return 1; }\nasync function b() { return 2; }\n",
            &[(RA, 1, 1, "high"), (RA, 2, 1, "high")],
        ),
        (
            "class without a parent",
            "a.ts",
            "class A { async m() { return 1; } }\n",
            &[(RA, 1, 11, "high")],
        ),
    ]);
}

#[test]
fn async_function_that_is_fine() {
    check(&[
        (
            "object method",
            "a.ts",
            "const o = { async m() { return 1; } };\n",
            &[],
        ),
        (
            "object property arrow",
            "a.ts",
            "const o = { m: async () => 1 };\n",
            &[],
        ),
        (
            "call argument",
            "a.ts",
            "app.get('/', async (req, res) => { res.send(1); });\n",
            &[],
        ),
        ("new argument", "a.ts", "new Foo(async () => 1);\n", &[]),
        (
            "typed declarator",
            "a.ts",
            "const f: Handler = async (req) => 1;\n",
            &[],
        ),
        (
            "returned function",
            "a.ts",
            "function make() { return async (c) => c.ok; }\n",
            &[],
        ),
        (
            "curried arrow",
            "a.ts",
            "const make = () => async (c) => c.ok;\n",
            &[],
        ),
        (
            "jsx attribute",
            "a.tsx",
            "const x = <button onClick={async () => { go(); }} />;\n",
            &[],
        ),
        (
            "declared Promise return",
            "a.ts",
            "async function f(): Promise<number> { return 1; }\n",
            &[],
        ),
        (
            "declared Promise return arrow",
            "a.ts",
            "const f = async (): Promise<number> => 1;\n",
            &[],
        ),
        (
            "declared Promise return method",
            "a.ts",
            "class A { async m(): Promise<void> { return; } }\n",
            &[],
        ),
        (
            "declared PromiseLike",
            "a.ts",
            "async function f(): PromiseLike<number> { return 1; }\n",
            &[],
        ),
        ("awaits", "a.ts", "async function f() { await g(); }\n", &[]),
        (
            "awaits a return",
            "a.ts",
            "async function f() { return await g(); }\n",
            &[],
        ),
        (
            "await using",
            "a.ts",
            "async function f() { await using x = g(); use(x); }\n",
            &[],
        ),
        (
            "for await",
            "a.ts",
            "async function f() { for await (const x of g()) { use(x); } }\n",
            &[],
        ),
        (
            "await in a nested block",
            "a.ts",
            "async function f() { if (a) { try { await g(); } catch {} } }\n",
            &[],
        ),
        ("not async", "a.ts", "function f() { return 1; }\n", &[]),
        (
            "generator",
            "a.ts",
            "async function* f() { yield 1; }\n",
            &[],
        ),
        ("empty body", "a.ts", "const f = async () => {};\n", &[]),
        (
            "only throws",
            "a.ts",
            "async function f() { throw new Error('todo'); }\n",
            &[],
        ),
        (
            "class with a parent",
            "a.ts",
            "class A extends B { async m() { return 1; } }\n",
            &[],
        ),
        (
            "class implementing",
            "a.ts",
            "class A implements I { async m() { return 1; } }\n",
            &[],
        ),
        (
            "class property arrow with a parent",
            "a.ts",
            "class A extends B { m = async () => 1; }\n",
            &[],
        ),
        (
            "override",
            "a.ts",
            "class A { override async m() { return 1; } }\n",
            &[],
        ),
        (
            "test file",
            "a.test.ts",
            "async function f() { return 1; }\n",
            &[],
        ),
        (
            "tests directory",
            "tests/a.ts",
            "async function f() { return 1; }\n",
            &[],
        ),
        (
            "no body",
            "a.ts",
            "declare function f(): Promise<void>;\n",
            &[],
        ),
        (
            "overload",
            "a.ts",
            "function f(a: string): Promise<void>;\nasync function f(a: any) { await g(a); }\n",
            &[],
        ),
        (
            "string and comment",
            "a.ts",
            "// async function f() { return 1; }\nconst s = 'async () => 1';\n",
            &[],
        ),
    ]);
}

#[test]
fn function_in_a_loop_that_shares_a_variable() {
    check(&[
        (
            "var counter",
            "a.js",
            "for (var i = 0; i < 3; i++) { fns.push(function () { return i; }); }\n",
            &[(LF, 1, 40, "high")],
        ),
        (
            "arrow with var counter",
            "a.js",
            "for (var i = 0; i < 3; i++) { setTimeout(() => log(i)); }\n",
            &[(LF, 1, 42, "high")],
        ),
        (
            "var outside, written in the loop",
            "a.js",
            "var x = 0;\nfor (const a of b) { fns.push(() => x); x++; }\n",
            &[(LF, 2, 31, "high")],
        ),
        (
            "var outside, written after the loop",
            "a.js",
            "var x = 0;\nwhile (c()) { fns.push(() => x); }\nx = 5;\n",
            &[(LF, 2, 24, "high")],
        ),
        (
            "var declared in the body",
            "a.js",
            "for (const a of b) { var v = a; fns.push(() => v); }\n",
            &[(LF, 1, 42, "high")],
        ),
        (
            "for-in var",
            "a.js",
            "for (var k in o) { cbs.push(() => k); }\n",
            &[(LF, 1, 29, "high")],
        ),
        (
            "do while",
            "a.js",
            "var n = 0;\ndo { cbs.push(() => n); n++; } while (n < 3);\n",
            &[(LF, 2, 15, "high")],
        ),
        (
            "function declaration",
            "a.js",
            "for (var i = 0; i < 3; i++) { function f() { return i; } use(f); }\n",
            &[(LF, 1, 31, "high")],
        ),
        (
            "parameter written after the loop",
            "a.js",
            "function g(n) { for (const a of b) { f(() => n); } n = 2; }\n",
            &[(LF, 1, 40, "high")],
        ),
        (
            "object method",
            "a.js",
            "for (var i = 0; i < 3; i++) { objs.push({ m() { return i; } }); }\n",
            &[(LF, 1, 43, "high")],
        ),
        (
            "outer function only, not the inner",
            "a.js",
            "for (var i = 0; i < 3; i++) {\n  f(function () {\n    return function () { return i; };\n  });\n}\n",
            &[(LF, 2, 5, "high")],
        ),
        (
            "function reassigned in the loop",
            "a.js",
            "function h() {}\nfor (const a of b) { cbs.push(() => h()); h = null; }\n",
            &[(LF, 2, 31, "high")],
        ),
        (
            "assigned inside the closure",
            "a.js",
            "var count = 0;\nfor (const a of b) { on(a, () => { count += 1; }); }\n",
            &[(LF, 2, 28, "high")],
        ),
    ]);
}

#[test]
fn function_in_a_loop_that_is_fine() {
    check(&[
        (
            "let outside, written in the loop",
            "a.js",
            "let x = 0;\nfor (const a of b) { fns.push(() => x); x++; }\n",
            &[],
        ),
        (
            "let outside, written after the loop",
            "a.js",
            "let x = 0;\nwhile (c()) { fns.push(() => x); }\nx = 5;\n",
            &[],
        ),
        (
            "let assigned inside the closure",
            "a.js",
            "let count = 0;\nfor (const a of b) { on(a, () => { count += 1; }); }\n",
            &[],
        ),
        (
            "let counter",
            "a.js",
            "for (let i = 0; i < 3; i++) { fns.push(() => i); }\n",
            &[],
        ),
        (
            "const of",
            "a.js",
            "for (const a of b) { fns.push(() => a); }\n",
            &[],
        ),
        (
            "let declared in the body",
            "a.js",
            "for (const a of b) { let v = a; fns.push(() => v); v = 2; }\n",
            &[],
        ),
        (
            "const outside",
            "a.js",
            "const c = 1;\nfor (var i = 0; i < 3; i++) { fns.push(() => c); }\n",
            &[],
        ),
        (
            "let outside never written",
            "a.js",
            "let x = 1;\nfor (const a of b) { fns.push(() => x); }\n",
            &[],
        ),
        (
            "no outside variable",
            "a.js",
            "for (var i = 0; i < 3; i++) { fns.push(function () { return 1; }); }\n",
            &[],
        ),
        (
            "function outside any loop",
            "a.js",
            "var i = 0;\ni++;\nconst f = () => i;\n",
            &[],
        ),
        (
            "immediately invoked",
            "a.js",
            "for (var i = 0; i < 3; i++) { (function () { use(i); })(); }\n",
            &[],
        ),
        (
            "immediately invoked arrow",
            "a.js",
            "for (var i = 0; i < 3; i++) { (() => use(i))(); }\n",
            &[],
        ),
        (
            "forEach callback",
            "a.js",
            "for (var i = 0; i < 3; i++) { arr.forEach((x) => use(x, i)); }\n",
            &[],
        ),
        (
            "map callback",
            "a.js",
            "for (var i = 0; i < 3; i++) { out.push(arr.map((x) => x + i)); }\n",
            &[],
        ),
        (
            "import is not a variable",
            "a.ts",
            "import { a } from 'm';\nfor (var i = 0; i < 3; i++) { fns.push(() => a); }\n",
            &[],
        ),
        (
            "type position only",
            "a.ts",
            "for (var i = 0; i < 3; i++) { fns.push((x: typeof i) => 1); }\n",
            &[],
        ),
        (
            "loop inside the function",
            "a.js",
            "var n = 0;\nconst f = () => { for (const a of b) { use(a); } return n; };\n",
            &[],
        ),
        (
            "in a string and a comment",
            "a.js",
            "for (var i = 0;;) { // () => i\n  s = '() => i'; }\n",
            &[],
        ),
    ]);
}

#[test]
fn every_synchronous_callback_method_is_left_alone() {
    for method in functions::SYNCHRONOUS_CALLBACKS {
        let source = format!("for (var i = 0; i < 3; i++) {{ arr.{method}((x) => use(x, i)); }}\n");
        assert_eq!(run("a.js", &source), Vec::new(), "{method}");
    }
    // A method that is not on the list can keep the callback for later.
    let source = "for (var i = 0; i < 3; i++) { arr.listen((x) => use(x, i)); }\n";
    assert_eq!(run("a.js", source).len(), 1);
}
