// Writes the differential fixtures for the Rust group C detectors (tree-sitter languages: Rust,
// Go, Dart, PHP) by running the TypeScript regex detectors over small sources.
// Run: bun scripts/parity/gen-group-c-fixtures.ts
//
// A case is "same" when the Rust detector must give exactly what the TypeScript one gave (rule,
// line, columns, confidence, edit), or "differs" when the tree is right and the regex is wrong; a
// "differs" case says why and states the Rust hits as [rule, line, startColumn].
import { writeFileSync } from 'node:fs';
import {
  type DetectorInput,
  RULE_DETECTORS,
} from '../../packages/engine/src/skills/engine/detectors.js';
import { lex } from '../../packages/engine/src/skills/engine/lex.js';

const RULES = ['no-unwrap-expect', 'no-unsafe-blocks', 'no-panic', 'avoid-print', 'no-eval'];

type Case = {
  name: string;
  file: string;
  source: string;
  why?: string;
  rust?: [string, number, number][];
};
const same = (name: string, file: string, source: string): Case => ({ name, file, source });
const differs = (
  name: string,
  file: string,
  source: string,
  why: string,
  rust: [string, number, number][],
): Case => ({ name, file, source, why, rust });

const cases: Case[] = [
  // --- Rust: no-unwrap-expect
  same('unwrap-basic', 'a.rs', 'fn f() { let x = a.unwrap(); }\n'),
  same('expect-basic', 'a.rs', 'fn f() { let x = a.expect("why"); }\n'),
  same('unwrap-chain', 'a.rs', 'fn f() { let x = a.b().c.unwrap(); }\n'),
  same('unwrap-multiline-chain', 'a.rs', 'fn f() {\n    let x = a\n        .unwrap();\n}\n'),
  same('unwrap-inner-space', 'a.rs', 'fn f() { a.unwrap( ); }\n'),
  same('unwrap-with-argument', 'a.rs', 'fn f() { a.unwrap(1); }\n'),
  same('unwrap-or-is-fine', 'a.rs', 'fn f() { a.unwrap_or(1); a.unwrap_or_else(|| 2); }\n'),
  same('unwrap-in-string-and-comment', 'a.rs', '// a.unwrap()\nfn f() { let s = "a.unwrap()"; }\n'),
  same('unwrap-in-macro', 'a.rs', 'fn f() { println!("{}", c.unwrap()); }\n'),
  same('expect-in-macro', 'a.rs', 'fn f() { assert_eq!(a.expect("x"), 1); }\n'),
  same('unwrap-two-on-a-line', 'a.rs', 'fn f() { a.unwrap(); b.unwrap(); }\n'),
  same('unwrap-user-method-expect', 'a.rs', 'fn f() { foo.expect(x); }\n'),
  same('unwrap-test-dir', 'tests/a.rs', 'fn f() { a.unwrap(); }\n'),
  same('unwrap-not-cfg-test', 'a.rs', '#[cfg(not(test))]\nfn f() { a.unwrap(); }\n'),
  same('unwrap-unicode-before', 'a.rs', 'fn f() { let s = "😀"; a.unwrap(); }\n'),
  same('unwrap-crlf', 'a.rs', 'fn f() {\r\n    a.unwrap();\r\n}\r\n'),
  same('unwrap-py-not-scanned', 'a.py', 'a.unwrap()\n'),
  same('unwrap-error-elsewhere', 'a.rs', 'fn f() { a.unwrap(); }\nfn g( {\n'),
  differs(
    'unwrap-spaced-dot',
    'a.rs',
    'fn f() { a . unwrap(); }\n',
    'a space around the dot is still a method call',
    [['no-unwrap-expect', 1, 12]],
  ),
  differs(
    'unwrap-in-cfg-test-module',
    'a.rs',
    'fn f() {}\n#[cfg(test)]\nmod t {\n    fn x() { a.unwrap(); }\n}\n',
    'code under #[cfg(test)] is test code, like a file under tests/',
    [],
  ),
  differs(
    'unwrap-in-test-fn',
    'a.rs',
    '#[test]\nfn x() { a.unwrap(); }\n',
    'a #[test] function is test code',
    [],
  ),
  differs(
    'unwrap-in-test-fn-stacked-attributes',
    'a.rs',
    '#[test]\n#[ignore]\nfn x() { a.unwrap(); }\n',
    'a #[test] function is test code, whatever else is stacked on it',
    [],
  ),
  differs(
    'unwrap-in-tokio-test',
    'a.rs',
    '#[tokio::test]\nasync fn x() { a.unwrap(); }\n',
    'a #[tokio::test] function is test code',
    [],
  ),

  // --- Rust: no-unsafe-blocks (tests are not skipped)
  same('unsafe-block', 'a.rs', 'fn f() { unsafe { g() } }\n'),
  same('unsafe-block-tight', 'a.rs', 'fn f() { unsafe{ g() } }\n'),
  same('unsafe-fn', 'a.rs', 'unsafe fn f() {}\n'),
  same('unsafe-pub-fn', 'a.rs', 'pub unsafe fn f() {}\n'),
  same('unsafe-pub-crate-fn', 'a.rs', 'pub(crate) unsafe fn f() {}\n'),
  same('unsafe-extern-fn-not-matched', 'a.rs', 'unsafe extern "C" fn f() {}\n'),
  same('unsafe-impl-not-matched', 'a.rs', 'unsafe impl Send for X {}\n'),
  same('unsafe-in-string-and-comment', 'a.rs', '// unsafe { }\nfn f() { let s = "unsafe { }"; }\n'),
  same('unsafe-in-macro', 'a.rs', 'fn f() { println!("{}", unsafe { h() }); }\n'),
  same('unsafe-test-dir-still-reported', 'tests/a.rs', 'fn f() { unsafe { g() } }\n'),
  same(
    'unsafe-in-cfg-test-still-reported',
    'a.rs',
    '#[cfg(test)]\nmod t { fn x() { unsafe { g() } } }\n',
  ),
  differs(
    'unsafe-brace-on-next-line',
    'a.rs',
    'fn f() {\n    unsafe\n    {\n        g()\n    }\n}\n',
    'the old pattern read one line at a time',
    [['no-unsafe-blocks', 2, 5]],
  ),

  // --- no-panic (Rust and Go; tests are skipped)
  same('rs-panic', 'a.rs', 'fn f() { panic!("x"); }\n'),
  same('rs-panic-scoped', 'a.rs', 'fn f() { std::panic!("x"); }\n'),
  same('rs-panic-brackets-not-matched', 'a.rs', 'fn f() { panic![]; }\n'),
  same(
    'rs-panic-in-string-and-comment',
    'a.rs',
    '// panic!("x")\nfn f() { let s = "panic!(x)"; }\n',
  ),
  same('rs-panic-test-dir', 'tests/a.rs', 'fn f() { panic!("x"); }\n'),
  same('rs-panic-in-macro', 'a.rs', 'fn f() { vec![panic!("x")]; }\n'),
  same('rs-panic-unwrap-same-line', 'a.rs', 'fn f() { a.unwrap(); panic!("x"); }\n'),
  differs(
    'rs-panic-in-test-fn',
    'a.rs',
    '#[test]\nfn x() { panic!("x"); }\n',
    'a #[test] function is test code',
    [],
  ),
  same('go-panic', 'a.go', 'package m\nfunc f() { panic("x") }\n'),
  same('go-panic-spaced', 'a.go', 'package m\nfunc f() { panic ("x") }\n'),
  same('go-method-panic', 'a.go', 'package m\nfunc f() { a.panic("y") }\n'),
  same('go-panic-in-string', 'a.go', 'package m\nfunc f() { s := "panic(" }\n'),
  same('go-panic-test-file', 'a_test.go', 'package m\nfunc f() { panic("x") }\n'),
  same('go-panic-defer', 'a.go', 'package m\nfunc f() { defer func() { panic(r) }() }\n'),
  differs(
    'go-panic-declaration',
    'a.go',
    'package m\nfunc panic(s string) {}\n',
    'a declaration is not a call',
    [],
  ),

  // --- measured on the corpus (docs/DETECTOR_QUALITY.md): the program's own `expect`, build scripts, Dart scripts
  differs(
    'expect-two-arguments',
    'a.rs',
    'fn f() { self.expect(&kind, "x")?; }\n',
    'Option::expect and Result::expect take one argument: a method of the program that takes two is its own',
    [],
  ),
  differs(
    'expect-three-arguments',
    'a.rs',
    'fn f() { p.expect(a, b, c); }\n',
    'same: more than one argument is not Option::expect',
    [],
  ),
  differs(
    'expect-no-arguments',
    'a.rs',
    'fn f() { p.expect(); }\n',
    'same: Option::expect always takes one argument',
    [],
  ),
  same('expect-trailing-comma', 'a.rs', 'fn f() { a.expect("x",); }\n'),
  same('expect-two-arguments-in-macro', 'a.rs', 'fn f() { println!("{}", p.expect(a, b)); }\n'),
  differs(
    'panic-in-build-script',
    'build.rs',
    'fn main() { panic!("x"); }\n',
    'a Cargo build script fails the build by panicking',
    [],
  ),
  differs(
    'panic-in-nested-build-script',
    'crates/x/build.rs',
    'fn main() { panic!("x"); }\n',
    'same, in a member crate',
    [],
  ),
  same('panic-in-file-ending-in-build', 'src/rebuild.rs', 'fn f() { panic!("x"); }\n'),
  same('panic-in-build-directory', 'build/lib.rs', 'fn f() { panic!("x"); }\n'),
  differs(
    'unwrap-in-build-script-still-reported',
    'build.rs',
    'fn main() { a.unwrap(); panic!("x"); }\n',
    'only the panic is exempt: the unwrap stays (its rule has no build script exemption)',
    [['no-unwrap-expect', 1, 14]],
  ),
  differs(
    'dart-print-in-example',
    'example/main.dart',
    "void main() { print('a'); }\n",
    'an example prints as its output',
    [],
  ),
  differs(
    'dart-print-in-tool',
    'tool/generate.dart',
    "void main() { print('a'); }\n",
    'a tool script prints as its output',
    [],
  ),
  differs(
    'dart-print-in-bin',
    'packages/cli/bin/cli.dart',
    "void main() { print('a'); }\n",
    'a Dart executable prints as its output',
    [],
  ),
  differs(
    'dart-print-in-examples',
    'examples/a/main.dart',
    "void main() { print('a'); }\n",
    'same, plural',
    [],
  ),
  same('dart-print-in-lib-toolbar', 'lib/toolbar/a.dart', "void main() { print('a'); }\n"),
  same(
    'dart-print-in-lib-examples-helper',
    'lib/examples_helper/a.dart',
    "void main() { print('a'); }\n",
  ),
  // --- Dart: avoid-print (tests are not skipped)
  same('dart-print', 'a.dart', "void main() { print('a'); }\n"),
  same('dart-print-spaced', 'a.dart', "void main() { print ('a'); }\n"),
  same('dart-method-print', 'a.dart', "void main() { a.print('b'); }\n"),
  same('dart-debug-print', 'a.dart', "void main() { debugPrint('c'); }\n"),
  same('dart-myprint', 'a.dart', "void main() { myprint('c'); }\n"),
  same(
    'dart-print-in-string-and-comment',
    'a.dart',
    "// print('a')\nvoid main() { var s = \"print('a')\"; }\n",
  ),
  same('dart-print-test-file', 'a_test.dart', "void main() { print('a'); }\n"),
  same('dart-print-unicode-before', 'a.dart', "void main() { var s = '😀'; print(s); }\n"),
  same('dart-print-multiline', 'a.dart', "void main() {\n  print(\n    'a',\n  );\n}\n"),
  differs(
    'dart-print-declaration',
    'a.dart',
    'void print(String s) {}\n',
    'a declaration is not a call',
    [],
  ),

  // --- PHP: no-eval
  same('php-eval', 'a.php', '<?php\neval($x);\n'),
  same('php-eval-spaced', 'a.php', '<?php\neval ($x);\n'),
  same('php-evalx', 'a.php', '<?php\nevalx($x);\n'),
  same('php-eval-uppercase-not-matched', 'a.php', '<?php\nEVAL($x);\n'),
  same('php-eval-in-comment-and-string', 'a.php', '<?php\n// eval($x)\n$s = "eval($x)";\n'),
  same('php-eval-assigned', 'a.php', '<?php\n$r = eval("return 1;");\n'),
  differs(
    'php-method-eval',
    'a.php',
    '<?php\n$o->eval($y);\n',
    'a method named eval is not the language construct',
    [],
  ),
  differs(
    'php-static-eval',
    'a.php',
    '<?php\nFoo::eval($z);\n',
    'a static method named eval is not the language construct',
    [],
  ),
];

const input = (c: Case): DetectorInput => {
  const lexed = lex(c.source, c.file);
  return {
    file: c.file,
    content: c.source,
    lineNumbers: undefined,
    masked: lexed.masked,
    strings: lexed.strings,
  };
};

const out = cases.map((c) => {
  const ts = RULES.flatMap((rule) =>
    (RULE_DETECTORS[rule]?.(input(c)) ?? []).map((h) => ({
      rule,
      line: h.index + 1,
      start: h.columns.start,
      end: h.columns.end,
      confidence: h.confidence,
      ...(h.edit ? { edit: h.edit } : {}),
    })),
  );
  return { ...c, ts };
});

writeFileSync(
  'crates/debuggatha-engine/tests/fixtures/group-c-conformance.json',
  `${JSON.stringify({ rules: RULES, cases: out }, null, 2)}\n`,
);
console.log(`${out.length} cases (${out.filter((c) => c.rust).length} differ)`);
