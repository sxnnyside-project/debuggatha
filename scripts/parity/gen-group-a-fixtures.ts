// Writes the differential fixtures for the Rust group A detectors (the syntax rules of the
// JavaScript/TypeScript family) by running the TypeScript regex detectors over small sources.
// Run: bun scripts/parity/gen-group-a-fixtures.ts
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

const RULES = [
  'no-var',
  'no-explicit-any',
  'strict-null-checks',
  'eqeqeq',
  'no-eval',
  'no-implied-eval',
  'no-extend-native',
  'prefer-object-spread',
  'prefer-template-literals',
  'no-dangerously-set-inner-html',
];

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
  // --- no-var
  same('var-basic', 'a.js', 'var a = 1;\n'),
  same('var-indent-and-spaces', 'a.ts', '    var   a = 1;\n'),
  same('var-for', 'a.js', 'for (var i = 0; i < 3; i++) {}\n'),
  same('var-export', 'a.ts', 'export var x = 1;\n'),
  same('var-in-comment-and-string', 'a.js', '// var x = 1\nconst s = "var x";\n'),
  same('var-two-declarators-one-hit', 'a.js', 'var a = 1, b = 2;\n'),
  same('var-identifier-containing-var', 'a.js', 'const myvar = 1; const variable = 2;\n'),
  same('var-mjs-cjs', 'a.mjs', 'var a = 1;\n'),
  same('var-unicode-before', 'a.js', 'const s = "😀"; var x = 1;\n'),
  same('var-dollar-underscore', 'a.js', 'var $a = 1; \n'),
  differs(
    'var-destructured-object',
    'a.js',
    'var { a } = obj;\n',
    'the old pattern needed a name after var',
    [['no-var', 1, 1]],
  ),
  differs(
    'var-destructured-array',
    'a.js',
    'var [a] = arr;\n',
    'the old pattern needed a name after var',
    [['no-var', 1, 1]],
  ),
  differs(
    'var-declare',
    'a.ts',
    'declare var VERSION: string;\n',
    'an ambient declaration describes a global, it is not code',
    [],
  ),
  differs(
    'var-declare-global',
    'a.ts',
    'export {};\ndeclare global {\n  var g: number;\n}\n',
    'var is the only way to declare a global',
    [],
  ),
  differs('var-then-newline', 'a.js', 'var\n  x = 1;\n', 'the lexer matched per line', [
    ['no-var', 1, 1],
  ]),
  same('var-vue-not-scanned', 'a.vue', '<script>\nvar a = 1;\n</script>\n'),

  // --- no-explicit-any (.ts, .tsx only)
  same('any-param', 'a.ts', 'function f(x: any) {}\n'),
  same('any-as', 'a.ts', 'const y = x as any;\n'),
  same('any-angle-assertion', 'a.ts', 'const z = <any>x;\n'),
  same('any-generic-arg', 'a.ts', 'const a: Array<any> = [];\n'),
  same('any-array', 'a.ts', 'let xs: any[] = [];\n'),
  same('any-union', 'a.ts', 'let u: any | null = null;\n'),
  same('any-catch', 'a.ts', 'try {} catch (e: any) {}\n'),
  same('any-in-string-and-comment', 'a.ts', '// x: any\nconst s = "a: any";\n'),
  same('any-js-file-no-types', 'a.js', 'const a = 1;\n'),
  same('any-tsx', 'a.tsx', 'const f = (p: any) => <p>{p}</p>;\n'),
  same('any-unknown-not-reported', 'a.ts', 'let k: unknown;\nconst anyone = 1;\n'),
  differs(
    'any-nested-generic',
    'a.ts',
    'const p: Promise<any[]> = f();\n',
    'the old pattern knew only : any, as any and <any>',
    [['no-explicit-any', 1, 18]],
  ),
  differs(
    'any-record',
    'a.ts',
    'type R = Record<string, any>;\n',
    'the old pattern knew only : any, as any and <any>',
    [['no-explicit-any', 1, 25]],
  ),
  differs(
    'any-constraint',
    'a.ts',
    'function g<T extends any>(t: T) {}\n',
    'the old pattern knew only : any, as any and <any>',
    [['no-explicit-any', 1, 22]],
  ),

  // --- strict-null-checks (.ts, .tsx)
  same('nonnull-member', 'a.ts', 'const n = user!.name;\n'),
  same('nonnull-paren', 'a.ts', 'const n = (a)!.b;\n'),
  same('nonnull-index', 'a.ts', 'const n = arr[0]!;\nconst m = x![0];\n'),
  same('nonnull-call', 'a.ts', 'foo(a!);\n'),
  same('not-nonnull-neq', 'a.ts', 'if (a !== b || c != d) {}\n'),
  same('not-nonnull-prefix', 'a.ts', 'const t = !x;\n'),
  same('nonnull-in-string', 'a.ts', 'const s = "hello!";\n'),
  differs(
    'nonnull-definite-field',
    'a.ts',
    'class A {\n  x!: string;\n}\n',
    'a definite assignment is not a non-null assertion',
    [],
  ),
  differs(
    'nonnull-definite-let',
    'a.ts',
    'let v!: number;\n',
    'a definite assignment is not a non-null assertion',
    [],
  ),
  differs('nonnull-jsx-text', 'a.tsx', 'const p = <p>Hello!</p>;\n', 'text in JSX is not code', []),
  differs(
    'nonnull-space-before-bang',
    'a.ts',
    'const n = user !.name;\n',
    'the old pattern needed the name right before the bang',
    [['strict-null-checks', 1, 15]],
  ),

  // --- eqeqeq
  same('eq-basic', 'a.js', 'if (a == b) {}\n'),
  same('neq-basic', 'a.ts', 'if (a != b) {}\n'),
  same('eq-strict-ok', 'a.js', 'if (a === b || c !== d) {}\n'),
  same('eq-null-right', 'a.js', 'if (a == null) {}\n'),
  same('eq-null-left', 'a.js', 'if (null == a) {}\n'),
  same('neq-null', 'a.js', 'if (a != null) {}\n'),
  same('eq-typeof', 'a.js', "if (typeof a == 'string') {}\n"),
  same('eq-undefined-reported', 'a.js', 'if (a == undefined) {}\n'),
  same('eq-in-string', 'a.js', 'const s = "a == b";\n'),
  same('eq-two-on-a-line', 'a.js', 'if (a == b && c != d) {}\n'),
  same('eq-lte-gte-arrow', 'a.js', 'const f = (a) => a <= b && a >= c;\n'),
  same('eq-next-line', 'a.js', 'const x = a\n  == b;\n'),
  differs(
    'eq-null-in-parens',
    'a.js',
    'if (x == (null)) {}\n',
    'null in parentheses is still null',
    [],
  ),

  // --- no-eval
  same('eval-basic', 'a.js', 'const v = eval(code);\n'),
  same('eval-space', 'a.js', 'eval ("x");\n'),
  same('eval-member-not-reported', 'a.js', 'window.eval(x); myeval(x);\n'),
  same('eval-in-string-and-comment', 'a.js', '// eval(x)\nconst s = "eval(x)";\n'),
  same('eval-optional-call', 'a.js', 'eval?.(x);\n'),
  same('timeout-string', 'a.js', 'setTimeout("go()", 10);\n'),
  same('interval-template', 'a.js', 'setInterval(`go()`, 10);\n'),
  same('timeout-function-ok', 'a.js', 'setTimeout(go, 10);\nsetTimeout(() => go(), 10);\n'),
  same('timeout-member-not-reported', 'a.js', 'obj.setTimeout("x", 1);\n'),
  same('eval-in-test-file', 'a.test.ts', 'eval(x);\n'),
  differs(
    'timeout-shadowed',
    'a.js',
    'const setTimeout = fakeTimers.setTimeout;\nsetTimeout("x", 1);\n',
    'a local setTimeout is not the global timer',
    [],
  ),

  // --- no-implied-eval
  same('new-function', 'a.js', 'const f = new Function("a", "return a");\n'),
  same('new-function-spaces', 'a.js', 'const f = new  Function (x);\n'),
  same('function-call-not-reported', 'a.js', 'const f = Function("x");\n'),
  same('new-function-no-parens', 'a.js', 'const f = new Function;\n'),
  same('new-function-in-tests-skipped', 'a.test.js', 'new Function("x");\n'),
  same('new-member-function', 'a.js', 'new window.Function("x");\n'),
  differs(
    'new-function-shadowed',
    'a.js',
    'class Function {}\nnew Function(1);\n',
    'a local class is not the global Function',
    [],
  ),

  // --- no-extend-native
  same('extend-array', 'a.js', 'Array.prototype.last = function () {};\n'),
  same('extend-string-nospace', 'a.js', 'String.prototype.x=1;\n'),
  same('extend-other-prototype', 'a.js', 'Foo.prototype.x = 1;\n'),
  same('extend-computed', 'a.js', "Array.prototype['x'] = 1;\n"),
  same('extend-compound', 'a.js', 'Array.prototype.x += 1;\n'),
  differs(
    'extend-comparison',
    'a.js',
    'if (Object.prototype.y == 1) {}\n',
    'a comparison is not an assignment',
    [['eqeqeq', 1, 24]],
  ),

  // --- prefer-object-spread
  same('assign-empty', 'a.js', 'const c = Object.assign({}, a);\n'),
  same('assign-empty-spaces', 'a.js', 'const c = Object.assign( { } , a);\n'),
  same('assign-only-empty', 'a.js', 'const c = Object.assign({});\n'),
  same('assign-target-ok', 'a.js', 'Object.assign(a, b);\nObject.assign({ a: 1 }, b);\n'),
  same('assign-paren-empty', 'a.js', 'const c = Object.assign(({}), a);\n'),
  differs(
    'assign-next-line',
    'a.js',
    'const c = Object.assign(\n  {},\n  a,\n);\n',
    'the lexer matched per line',
    [['prefer-object-spread', 1, 11]],
  ),

  // --- prefer-template-literals
  same('concat-string-then-name', 'a.js', 'const s = "a" + b;\n'),
  same('concat-name-then-string', 'a.js', 'const s = a + "b";\n'),
  same('concat-member-then-string', 'a.js', 'const s = a.b + "x";\n'),
  same('concat-string-number', 'a.js', 'const s = "a" + 1;\n'),
  same('concat-two-strings', 'a.js', 'const s = "a" + "b";\n'),
  same('concat-number-string', 'a.js', 'const s = 1 + "a";\n'),
  same('concat-call-then-string', 'a.js', 'const s = f() + "a";\n'),
  same('concat-chain-one-hit', 'a.js', 'const s = "a" + b + "c";\n'),
  same('concat-template-then-name', 'a.js', 'const s = `a` + b;\n'),
  same('concat-name-then-template', 'a.js', 'const s = a + `b`;\n'),
  same('concat-paren-right', 'a.js', 'const s = "a" + (b);\n'),
  same('concat-number-then-sum', 'a.js', 'const s = n + 1 + "px";\n'),
  same('concat-numbers-only', 'a.js', 'const n = a + b;\n'),
  differs('concat-next-line', 'a.js', 'const s = "a" +\n  b;\n', 'the lexer matched per line', [
    ['prefer-template-literals', 1, 11],
  ]),

  // --- no-dangerously-set-inner-html (.tsx, .jsx)
  same(
    'dangerous-jsx-attribute',
    'a.tsx',
    'const a = <div dangerouslySetInnerHTML={{ __html: x }} />;\n',
  ),
  same('dangerous-object-key', 'a.jsx', 'const p = { dangerouslySetInnerHTML: { __html: x } };\n'),
  same(
    'dangerous-in-comment-and-string',
    'a.tsx',
    '// dangerouslySetInnerHTML\nconst s = "dangerouslySetInnerHTML";\n',
  ),
  same('dangerous-js-not-scanned', 'a.js', 'const p = { dangerouslySetInnerHTML: 1 };\n'),
  same('dangerous-ts-not-scanned', 'a.ts', 'const p = { dangerouslySetInnerHTML: 1 };\n'),
  differs(
    'dangerous-jsx-text',
    'a.tsx',
    'const a = <p>dangerouslySetInnerHTML</p>;\n',
    'text in JSX is not code',
    [],
  ),

  // --- layout
  same('crlf-lines', 'a.js', 'var a = 1;\r\nif (a == b) {}\r\n'),
  same('recoverable-error-elsewhere', 'a.js', 'const q;\nvar b = 1;\n'),
  differs(
    'fatal-error-file-is-not-analyzed',
    'a.js',
    'const q = ;\nvar b = 1;\n',
    'nothing is concluded about a file the parser could not read',
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
  'crates/debuggatha-engine/tests/fixtures/group-a-conformance.json',
  `${JSON.stringify({ rules: RULES, cases: out }, null, 2)}\n`,
);
console.log(`${out.length} cases (${out.filter((c) => c.rust).length} differ)`);
