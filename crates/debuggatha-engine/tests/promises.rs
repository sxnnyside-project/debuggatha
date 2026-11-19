//! `no-floating-promises` is specified by the cases below. Each is a set of
//! files (the review), the file under test, and the expected hits `(line, column, confidence)`.
//! A positive says what the tree proves is a promise; a negative is a look-alike that must stay
//! quiet. Mutations of the detector must fail at least one of them.
#![allow(clippy::expect_used, clippy::unwrap_used)]
// a failed expect is a failed test
// The tests are tables of cases: splitting a table only hides what it covers.
#![allow(clippy::too_many_lines)]

use std::path::Path;

use debuggatha_engine::detect::modules::ModuleIndex;
use debuggatha_engine::detect::{DetectorInput, SourceUnit, promises};
use debuggatha_engine::syntax::parse;

type Expected<'a> = &'a [(u32, u32, &'a str)];
type Files<'a> = &'a [(&'a str, &'a str)];

fn run(target: &str, files: Files<'_>, indexed: bool) -> Vec<(u32, u32, String)> {
    let units: Vec<SourceUnit> = files
        .iter()
        .map(|(file, content)| SourceUnit {
            file: (*file).to_owned(),
            content: (*content).to_owned(),
            changed_lines: None,
        })
        .collect();
    let index = ModuleIndex::build(&units);
    let source = files
        .iter()
        .find(|(f, _)| *f == target)
        .expect("the target is one of the files")
        .1;
    let syntax = parse(Path::new(target), source).expect("a supported language");
    let input =
        DetectorInput::new(target, source, Some(&syntax)).with_modules(indexed.then_some(&index));
    promises::detect(&input)
        .into_iter()
        .map(|(rule, h)| {
            assert_eq!(rule, "no-floating-promises");
            (
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

fn check(cases: &[(&str, &str, Files<'_>, Expected<'_>)]) {
    let mut bad = Vec::new();
    for (name, target, files, want) in cases {
        let got = run(target, files, true);
        let want: Vec<(u32, u32, String)> = want
            .iter()
            .map(|(l, c, k)| (*l, *c, (*k).to_owned()))
            .collect();
        if got != want {
            bad.push(format!("{name}: got {got:?}, want {want:?}"));
        }
    }
    assert!(bad.is_empty(), "\n{}", bad.join("\n"));
}

#[test]
fn a_call_to_an_async_function_of_the_file() {
    check(&[
        (
            "declaration",
            "a.ts",
            &[("a.ts", "async function f() { return 1; }\nf();\n")],
            &[(2, 1, "high")],
        ),
        (
            "const arrow",
            "a.ts",
            &[("a.ts", "const f = async () => 1;\nf();\n")],
            &[(2, 1, "high")],
        ),
        (
            "let never reassigned",
            "a.ts",
            &[("a.ts", "let f = async () => 1;\nf();\n")],
            &[(2, 1, "high")],
        ),
        (
            "let reassigned: the tree cannot say which function runs",
            "a.ts",
            &[("a.ts", "let f = async () => 1;\nf = () => 1;\nf();\n")],
            &[],
        ),
        (
            "plain JavaScript file",
            "a.js",
            &[("a.js", "async function f() {}\nf();\n")],
            &[(2, 1, "high")],
        ),
        (
            "a declared Promise return type is the author's contract",
            "a.ts",
            &[(
                "a.ts",
                "function f(): Promise<void> { return g(); }\nf();\n",
            )],
            &[(2, 1, "high")],
        ),
        (
            "an arrow that declares Promise",
            "a.ts",
            &[("a.ts", "const f = (): Promise<number> => g();\nf();\n")],
            &[(2, 1, "high")],
        ),
        (
            "an overload with no body that declares Promise",
            "a.ts",
            &[("a.ts", "declare function f(): Promise<void>;\nf();\n")],
            &[(2, 1, "high")],
        ),
        (
            "an arrow whose body is new Promise",
            "a.ts",
            &[(
                "a.ts",
                "const delay = (t) => new Promise((r) => setTimeout(r, t));\ndelay(0);\n",
            )],
            &[(2, 1, "high")],
        ),
        (
            "the sleep idiom",
            "a.ts",
            &[(
                "a.ts",
                "function sleep(t) {\n  return new Promise((r) => setTimeout(r, t));\n}\nsleep(1);\n",
            )],
            &[(4, 1, "high")],
        ),
        (
            "a block arrow that returns Promise.resolve",
            "a.ts",
            &[(
                "a.ts",
                "const f = () => {\n  return Promise.resolve(1);\n};\nf();\n",
            )],
            &[(4, 1, "high")],
        ),
        (
            "a function that returns a promise only sometimes",
            "a.ts",
            &[(
                "a.ts",
                "function f(x) {\n  if (x) return 1;\n  return new Promise(() => {});\n}\nf();\n",
            )],
            &[],
        ),
        (
            "a bare return and a promise",
            "a.ts",
            &[(
                "a.ts",
                "function f(x) {\n  if (x) return;\n  return new Promise(() => {});\n}\nf();\n",
            )],
            &[],
        ),
        (
            "returns of an inner function do not count",
            "a.ts",
            &[(
                "a.ts",
                "function f() {\n  const g = () => 1;\n  return new Promise(() => {});\n}\nf();\n",
            )],
            &[(5, 1, "high")],
        ),
        (
            "a promise returned only by an inner function",
            "a.ts",
            &[(
                "a.ts",
                "function f() {\n  const g = () => new Promise(() => {});\n  return g;\n}\nf();\n",
            )],
            &[],
        ),
        (
            "a promise returned by an inner function is not the outer one's",
            "a.ts",
            &[(
                "a.ts",
                "function f() {\n  function g() { return new Promise(() => {}); }\n  const h = () => { return new Promise(() => {}); };\n}\nf();\n",
            )],
            &[],
        ),
        (
            "an inner function that returns something else does not spoil the outer one",
            "a.ts",
            &[(
                "a.ts",
                "function f() {\n  const g = () => { return 1; };\n  function h() { return 2; }\n  return new Promise(() => {});\n}\nf();\n",
            )],
            &[(6, 1, "high")],
        ),
        (
            "an arrow that builds something else with new",
            "a.ts",
            &[("a.ts", "const mk = () => new Foo();\nmk();\n")],
            &[],
        ),
        (
            "an arrow that calls a Promise method that is not one",
            "a.ts",
            &[("a.ts", "const mk = () => Promise.foo();\nmk();\n")],
            &[],
        ),
        (
            "a function with no return",
            "a.ts",
            &[(
                "a.ts",
                "function f() {\n  new Promise(() => {});\n}\nf();\n",
            )],
            &[(2, 3, "high")],
        ),
        (
            "a synchronous function",
            "a.ts",
            &[("a.ts", "function f() { return 1; }\nf();\n")],
            &[],
        ),
        (
            "a parameter that shadows the async function",
            "a.ts",
            &[("a.ts", "async function f() {}\nfunction g(f) { f(); }\n")],
            &[],
        ),
        (
            "a function whose return type is another type",
            "a.ts",
            &[("a.ts", "function f(): Array<number> { return []; }\nf();\n")],
            &[],
        ),
        (
            "nested in a callback",
            "a.ts",
            &[(
                "a.ts",
                "const f = async () => 1;\narr.forEach(() => {\n  f();\n});\n",
            )],
            &[(3, 3, "high")],
        ),
        (
            "two calls on a line are one hit",
            "a.ts",
            &[("a.ts", "async function f() {}\nf(); f();\n")],
            &[(2, 1, "high")],
        ),
        (
            "a statement over several lines is reported where it starts",
            "a.ts",
            &[("a.ts", "async function f() {}\nf()\n  .then(x);\n")],
            &[(2, 1, "high")],
        ),
    ]);
}

#[test]
fn what_the_language_gives_us() {
    check(&[
        (
            "Promise.all",
            "a.ts",
            &[("a.ts", "Promise.all([a, b]);\n")],
            &[(1, 1, "high")],
        ),
        (
            "Promise.resolve and reject",
            "a.ts",
            &[("a.ts", "Promise.resolve(1);\nPromise.reject(e);\n")],
            &[(1, 1, "high"), (2, 1, "high")],
        ),
        (
            "a Promise method that is not a promise",
            "a.ts",
            &[("a.ts", "Promise.prototype.then.call(a);\nPromise.foo();\n")],
            &[],
        ),
        (
            "a local Promise",
            "a.ts",
            &[(
                "a.ts",
                "const Promise = { resolve() {} };\nPromise.resolve();\n",
            )],
            &[],
        ),
        (
            "fetch",
            "a.ts",
            &[("a.ts", "fetch('/x');\n")],
            &[(1, 1, "high")],
        ),
        (
            "a local fetch",
            "a.ts",
            &[("a.ts", "function fetch() {}\nfetch();\n")],
            &[],
        ),
        (
            "new Promise then",
            "a.ts",
            &[("a.ts", "new Promise((r) => r()).then(x);\n")],
            &[(1, 1, "high")],
        ),
        (
            "new Promise",
            "a.ts",
            &[("a.ts", "new Promise((r) => r());\n")],
            &[(1, 1, "high")],
        ),
        (
            "a local Promise class",
            "a.ts",
            &[("a.ts", "class Promise {}\nnew Promise();\n")],
            &[],
        ),
        (
            "dynamic import",
            "a.ts",
            &[("a.ts", "import('./x');\n")],
            &[(1, 1, "high")],
        ),
        (
            "an async arrow called on the spot",
            "a.ts",
            &[("a.ts", "(async () => { await x; })();\n")],
            &[(1, 1, "high")],
        ),
        (
            "an async function expression called on the spot",
            "a.ts",
            &[("a.ts", "(async function () { await x; })();\n")],
            &[(1, 1, "high")],
        ),
        (
            "a synchronous function called on the spot",
            "a.ts",
            &[("a.ts", "(() => { x(); })();\n")],
            &[],
        ),
        (
            "fetch then",
            "a.ts",
            &[("a.ts", "fetch(u).then((r) => r.json());\n")],
            &[(1, 1, "high")],
        ),
    ]);
}

#[test]
fn what_handles_a_promise() {
    check(&[
        (
            "await",
            "a.ts",
            &[(
                "a.ts",
                "async function f() {}\nasync function g() { await f(); }\n",
            )],
            &[],
        ),
        (
            "void",
            "a.ts",
            &[("a.ts", "async function f() {}\nvoid f();\n")],
            &[],
        ),
        (
            "return, assignment, argument, array",
            "a.ts",
            &[(
                "a.ts",
                "async function f() {}\nfunction g() { return f(); }\nconst p = f();\nh(f());\n[f()];\n",
            )],
            &[],
        ),
        (
            "catch",
            "a.ts",
            &[("a.ts", "async function f() {}\nf().catch(h);\n")],
            &[],
        ),
        (
            "then with a rejection handler",
            "a.ts",
            &[("a.ts", "async function f() {}\nf().then(a, b);\n")],
            &[],
        ),
        (
            "then chain that ends in catch",
            "a.ts",
            &[("a.ts", "async function f() {}\nf().then(a).catch(h);\n")],
            &[],
        ),
        (
            "then with no handler",
            "a.ts",
            &[("a.ts", "async function f() {}\nf().then(a);\n")],
            &[(2, 1, "high")],
        ),
        (
            "two thens",
            "a.ts",
            &[("a.ts", "async function f() {}\nf().then(a).then(b);\n")],
            &[(2, 1, "high")],
        ),
        (
            "finally does not catch",
            "a.ts",
            &[("a.ts", "async function f() {}\nf().finally(z);\n")],
            &[(2, 1, "high")],
        ),
        (
            "a then after a catch can still reject",
            "a.ts",
            &[("a.ts", "async function f() {}\nf().catch(h).then(a);\n")],
            &[(2, 1, "high")],
        ),
        (
            "a catch with no handler",
            "a.ts",
            &[("a.ts", "async function f() {}\nf().catch();\n")],
            &[(2, 1, "high")],
        ),
    ]);
}

#[test]
fn a_value_that_is_thrown_away_in_a_larger_expression() {
    check(&[
        (
            "and",
            "a.ts",
            &[("a.ts", "async function f() {}\nok && f();\n")],
            &[(2, 1, "high")],
        ),
        (
            "or, on the left",
            "a.ts",
            &[("a.ts", "async function f() {}\nf() || g;\n")],
            &[(2, 1, "high")],
        ),
        (
            "conditional",
            "a.ts",
            &[("a.ts", "async function f() {}\nok ? f() : 1;\n")],
            &[(2, 1, "high")],
        ),
        (
            "conditional, the other branch",
            "a.ts",
            &[("a.ts", "async function f() {}\nok ? 1 : f();\n")],
            &[(2, 1, "high")],
        ),
        (
            "conditional with no promise",
            "a.ts",
            &[("a.ts", "async function f() {}\nok ? 1 : 2;\n")],
            &[],
        ),
        (
            "sequence, last",
            "a.ts",
            &[("a.ts", "async function f() {}\na, f();\n")],
            &[(2, 1, "high")],
        ),
        (
            "sequence, not last",
            "a.ts",
            &[("a.ts", "async function f() {}\n(f(), 1);\n")],
            &[],
        ),
        (
            "optional call",
            "a.ts",
            &[("a.ts", "async function f() {}\nf?.();\n")],
            &[(2, 1, "high")],
        ),
        (
            "type assertion and non-null",
            "a.ts",
            &[(
                "a.ts",
                "async function f() {}\nf() as unknown;\nf()!;\n(f() satisfies unknown);\n",
            )],
            &[(2, 1, "high"), (3, 1, "high"), (4, 1, "high")],
        ),
    ]);
}

#[test]
fn a_then_whose_origin_is_not_verified() {
    check(&[
        (
            "unknown receiver",
            "a.ts",
            &[("a.ts", "x.then(cb);\n")],
            &[(1, 1, "low")],
        ),
        (
            "unknown call as receiver",
            "a.ts",
            &[("a.ts", "foo().then(cb);\n")],
            &[(1, 1, "low")],
        ),
        (
            "with a rejection handler",
            "a.ts",
            &[("a.ts", "x.then(cb, err);\n")],
            &[],
        ),
        ("no callback", "a.ts", &[("a.ts", "x.then();\n")], &[]),
        (
            "catch and finally of an unknown receiver say nothing",
            "a.ts",
            &[("a.ts", "x.catch(h);\nx.catch();\nx.finally(f);\n")],
            &[],
        ),
        (
            "a chain on an unknown receiver: the last then decides",
            "a.ts",
            &[("a.ts", "x.then(a).then(b);\n")],
            &[(1, 1, "low")],
        ),
        (
            "assigned",
            "a.ts",
            &[("a.ts", "const p = x.then(cb);\n")],
            &[],
        ),
    ]);
}

#[test]
fn a_method_of_the_enclosing_class() {
    check(&[
        (
            "this.m()",
            "a.ts",
            &[(
                "a.ts",
                "class A {\n  async m() {}\n  run() {\n    this.m();\n  }\n}\n",
            )],
            &[(4, 5, "high")],
        ),
        (
            "a class with a parent: an override could change it, which is not assumed",
            "a.ts",
            &[(
                "a.ts",
                "class A extends B {\n  async m() {}\n  run() {\n    this.m();\n  }\n}\n",
            )],
            &[(4, 5, "high")],
        ),
        (
            "an arrow keeps this",
            "a.ts",
            &[(
                "a.ts",
                "class A {\n  async m() {}\n  run() {\n    [1].forEach(() => {\n      this.m();\n    });\n  }\n}\n",
            )],
            &[(5, 7, "high")],
        ),
        (
            "a plain function does not",
            "a.ts",
            &[(
                "a.ts",
                "class A {\n  async m() {}\n  run() {\n    [1].forEach(function () {\n      this.m();\n    });\n  }\n}\n",
            )],
            &[],
        ),
        (
            "an object method does not",
            "a.ts",
            &[(
                "a.ts",
                "class A {\n  async m() {}\n  run() {\n    return { x() { this.m(); } };\n  }\n}\n",
            )],
            &[],
        ),
        (
            "a property holding an async arrow",
            "a.ts",
            &[(
                "a.ts",
                "class A {\n  m = async () => 1;\n  run() {\n    this.m();\n  }\n}\n",
            )],
            &[(4, 5, "high")],
        ),
        (
            "a method that is not async",
            "a.ts",
            &[(
                "a.ts",
                "class A {\n  m() {}\n  run() {\n    this.m();\n  }\n}\n",
            )],
            &[],
        ),
        (
            "a static method is not on this",
            "a.ts",
            &[(
                "a.ts",
                "class A {\n  static async m() {}\n  run() {\n    this.m();\n  }\n}\n",
            )],
            &[],
        ),
        (
            "a method of the class declared with Promise",
            "a.ts",
            &[(
                "a.ts",
                "class A {\n  m(): Promise<void> { return g(); }\n  run() {\n    this.m();\n  }\n}\n",
            )],
            &[(4, 5, "high")],
        ),
        (
            "an instance of the class needs types",
            "a.ts",
            &[(
                "a.ts",
                "class A {\n  async m() {}\n}\nnew A().m();\nconst a = new A();\na.m();\n",
            )],
            &[],
        ),
        (
            "a getter that declares Promise is not a method either",
            "a.ts",
            &[(
                "a.ts",
                "class A {\n  get m(): Promise<void> { return g(); }\n  run() {\n    this.m();\n  }\n}\n",
            )],
            &[],
        ),
        (
            "a getter is not a method",
            "a.ts",
            &[(
                "a.ts",
                "class A {\n  get m() { return 1; }\n  run() {\n    this.m();\n  }\n}\n",
            )],
            &[],
        ),
    ]);
}

#[test]
fn an_import_resolved_through_the_module_index() {
    let store: (&str, &str) = (
        "src/store.ts",
        "export async function save() {}\nexport function sync() {}\nexport const load = async () => 1;\nconst local = async () => 1;\nexport { local as stored };\nexport default async function () {}\n",
    );
    check(&[
        (
            "named",
            "src/a.ts",
            &[
                store,
                ("src/a.ts", "import { save } from './store';\nsave();\n"),
            ],
            &[(2, 1, "high")],
        ),
        (
            "renamed on import",
            "src/a.ts",
            &[
                store,
                ("src/a.ts", "import { save as s } from './store';\ns();\n"),
            ],
            &[(2, 1, "high")],
        ),
        (
            "an export of a variable",
            "src/a.ts",
            &[
                store,
                ("src/a.ts", "import { load } from './store';\nload();\n"),
            ],
            &[(2, 1, "high")],
        ),
        (
            "an export list that renames a local",
            "src/a.ts",
            &[
                store,
                ("src/a.ts", "import { stored } from './store';\nstored();\n"),
            ],
            &[(2, 1, "high")],
        ),
        (
            "default",
            "src/a.ts",
            &[
                store,
                ("src/a.ts", "import save from './store';\nsave();\n"),
            ],
            &[(2, 1, "high")],
        ),
        (
            "a synchronous export",
            "src/a.ts",
            &[
                store,
                ("src/a.ts", "import { sync } from './store';\nsync();\n"),
            ],
            &[],
        ),
        (
            "a name the file does not export",
            "src/a.ts",
            &[
                store,
                ("src/a.ts", "import { nope } from './store';\nnope();\n"),
            ],
            &[],
        ),
        (
            "a namespace",
            "src/a.ts",
            &[
                store,
                (
                    "src/a.ts",
                    "import * as store from './store';\nstore.save();\nstore.sync();\n",
                ),
            ],
            &[(2, 1, "high")],
        ),
        (
            "a type-only import cannot be called",
            "src/a.ts",
            &[
                store,
                (
                    "src/a.ts",
                    "import type { save } from './store';\nsave();\n",
                ),
            ],
            &[],
        ),
        (
            "an inline type specifier cannot be called",
            "src/a.ts",
            &[
                store,
                (
                    "src/a.ts",
                    "import { type save } from './store';\nsave();\n",
                ),
            ],
            &[],
        ),
        (
            "a type-only default and namespace",
            "src/a.ts",
            &[
                store,
                (
                    "src/a.ts",
                    "import type save from './store';\nimport type * as ns from './store';\nsave();\nns.save();\n",
                ),
            ],
            &[],
        ),
        (
            "an import used in a then",
            "src/a.ts",
            &[
                store,
                (
                    "src/a.ts",
                    "import { save } from './store';\nsave().then(x);\n",
                ),
            ],
            &[(2, 1, "high")],
        ),
        (
            "a package is not resolved",
            "src/a.ts",
            &[
                store,
                ("src/a.ts", "import { save } from 'store';\nsave();\n"),
            ],
            &[],
        ),
        (
            "an alias is not resolved",
            "src/a.ts",
            &[
                store,
                ("src/a.ts", "import { save } from '@/store';\nsave();\n"),
            ],
            &[],
        ),
        (
            "a file that is not in the review",
            "src/a.ts",
            &[("src/a.ts", "import { save } from './store';\nsave();\n")],
            &[],
        ),
        (
            "the extension a compiled file carries",
            "src/a.ts",
            &[
                store,
                ("src/a.ts", "import { save } from './store.js';\nsave();\n"),
            ],
            &[(2, 1, "high")],
        ),
        (
            "an index file of a folder",
            "src/a.ts",
            &[
                ("src/lib/index.ts", "export async function boot() {}\n"),
                ("src/a.ts", "import { boot } from './lib';\nboot();\n"),
            ],
            &[(2, 1, "high")],
        ),
        (
            "a parent folder",
            "src/deep/a.ts",
            &[
                store,
                (
                    "src/deep/a.ts",
                    "import { save } from '../store';\nsave();\n",
                ),
            ],
            &[(2, 1, "high")],
        ),
        (
            "a path that climbs out of the repository",
            "a.ts",
            &[
                ("store.ts", "export async function save() {}\n"),
                ("a.ts", "import { save } from '../store';\nsave();\n"),
            ],
            &[],
        ),
        (
            "through a barrel with a named re-export",
            "src/a.ts",
            &[
                store,
                ("src/barrel.ts", "export { save } from './store';\n"),
                ("src/a.ts", "import { save } from './barrel';\nsave();\n"),
            ],
            &[(2, 1, "high")],
        ),
        (
            "through a barrel with a renamed re-export",
            "src/a.ts",
            &[
                store,
                (
                    "src/barrel.ts",
                    "export { save as persist } from './store';\n",
                ),
                (
                    "src/a.ts",
                    "import { persist } from './barrel';\npersist();\n",
                ),
            ],
            &[(2, 1, "high")],
        ),
        (
            "through a barrel with export star",
            "src/a.ts",
            &[
                store,
                ("src/barrel.ts", "export * from './store';\n"),
                ("src/a.ts", "import { save } from './barrel';\nsave();\n"),
            ],
            &[(2, 1, "high")],
        ),
        (
            "export star does not carry the default",
            "src/a.ts",
            &[
                store,
                ("src/barrel.ts", "export * from './store';\n"),
                ("src/a.ts", "import save from './barrel';\nsave();\n"),
            ],
            &[],
        ),
        (
            "a default re-exported by name",
            "src/a.ts",
            &[
                store,
                (
                    "src/barrel.ts",
                    "export { default as save } from './store';\n",
                ),
                ("src/a.ts", "import { save } from './barrel';\nsave();\n"),
            ],
            &[(2, 1, "high")],
        ),
        (
            "a default that names a local",
            "src/a.ts",
            &[
                (
                    "src/store.ts",
                    "const go = async () => 1;\nexport default go;\n",
                ),
                ("src/a.ts", "import go from './store';\ngo();\n"),
            ],
            &[(2, 1, "high")],
        ),
        (
            "a default arrow",
            "src/a.ts",
            &[
                ("src/store.ts", "export default async () => 1;\n"),
                ("src/a.ts", "import go from './store';\ngo();\n"),
            ],
            &[(2, 1, "high")],
        ),
        (
            "export star as a namespace names no function",
            "src/a.ts",
            &[
                store,
                ("src/barrel.ts", "export * as ns from './store';\n"),
                ("src/a.ts", "import { save } from './barrel';\nsave();\n"),
            ],
            &[],
        ),
        (
            "a cycle of export stars ends",
            "src/a.ts",
            &[
                ("src/x.ts", "export * from './y';\n"),
                ("src/y.ts", "export * from './x';\n"),
                ("src/a.ts", "import { nope } from './x';\nnope();\n"),
            ],
            &[],
        ),
        (
            "the sleep idiom exported",
            "src/a.ts",
            &[
                (
                    "src/util.ts",
                    "export function sleep(t) { return new Promise((r) => setTimeout(r, t)); }\n",
                ),
                ("src/a.ts", "import { sleep } from './util';\nsleep(1);\n"),
            ],
            &[(2, 1, "high")],
        ),
        (
            "a declared Promise type is a promise in another file",
            "src/a.ts",
            &[
                (
                    "src/store.ts",
                    "export function save(): Promise<void> { return g(); }\n",
                ),
                ("src/a.ts", "import { save } from './store';\nsave();\n"),
            ],
            &[(2, 1, "high")],
        ),
    ]);
}

#[test]
fn without_the_index_an_import_says_nothing() {
    let files: Files<'_> = &[
        ("src/store.ts", "export async function save() {}\n"),
        ("src/a.ts", "import { save } from './store';\nsave();\n"),
    ];
    assert_eq!(run("src/a.ts", files, false), vec![]);
    assert_eq!(run("src/a.ts", files, true).len(), 1);
}

#[test]
fn an_unparsable_file_is_left_out_of_the_index() {
    check(&[(
        "a store with a syntax error that stops the parse",
        "src/a.ts",
        &[
            ("src/store.ts", "export async function save( {{{\n"),
            ("src/a.ts", "import { save } from './store';\nsave();\n"),
        ],
        &[],
    )]);
}

#[test]
fn only_an_unverified_origin_lowers_the_severity() {
    use debuggatha_engine::detect::{DetectorInput, promises};
    let source = "async function f() {}\nf();\nx.then(cb);\n";
    let syntax = parse(Path::new("a.ts"), source).expect("a supported language");
    let input = DetectorInput::new("a.ts", source, Some(&syntax));
    let severities: Vec<_> = promises::detect(&input)
        .into_iter()
        .map(|(_, h)| h.severity)
        .collect();
    // Proven: the rule's own severity. Unverified: medium.
    assert_eq!(
        severities,
        [None, Some(debuggatha_core::review_engine::Severity::Medium)]
    );
}
