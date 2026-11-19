// Writes the differential fixtures for the Rust inline suppression by running the TypeScript
// implementation (the parity reference, ADR-0006). Run: bun scripts/parity/gen-suppress-fixtures.ts
//
// For each source it records the comments the TypeScript lexer found, the directives read from
// them, and which directive (by index) covers each probed (line, rule). The Rust test feeds the
// same comments to its parser, and separately checks that its own comment extraction (from the
// syntax tree) gives those comments for JavaScript and TypeScript.
import { writeFileSync } from 'node:fs';
import { lex } from '../../packages/engine/src/skills/engine/lex.js';
import {
  findDirective,
  inlineSuppressionComment,
  parseDirectives,
} from '../../packages/engine/src/skills/engine/suppress.js';

const cases: { name: string; file: string; source: string; note?: string }[] = [
  {
    name: 'line',
    file: 'a.ts',
    source: 'risky(); // debuggatha-ignore no-eval -- sandboxed loader',
  },
  {
    name: 'next-line',
    file: 'a.ts',
    source: '// debuggatha-ignore-next-line eqeqeq\nif (a == b) {}\nif (c == d) {}',
  },
  {
    name: 'file',
    file: 'a.ts',
    source: '// debuggatha-ignore-file no-explicit-any -- generated\nlet a: any;',
  },
  { name: 'unnamed-rule', file: 'a.ts', source: 'x(); // debuggatha-ignore no-eval' },
  { name: 'all-rules-bare', file: 'a.ts', source: 'x(); // debuggatha-ignore' },
  { name: 'all-rules-star', file: 'a.ts', source: 'x(); // debuggatha-ignore * -- why' },
  {
    name: 'several-ids',
    file: 'a.ts',
    source: 'x(); // debuggatha-ignore no-eval, no-var eqeqeq -- legacy',
  },
  { name: 'star-among-ids', file: 'a.ts', source: 'x(); // debuggatha-ignore no-eval * -- r' },
  { name: 'colon-ids', file: 'a.ts', source: 'x(); // debuggatha-ignore: no-eval:no-var -- r' },
  { name: 'hash-comment', file: 'a.py', source: 'x = eval(y)  # debuggatha-ignore no-eval -- why' },
  { name: 'block-comment', file: 'a.ts', source: 'x(); /* debuggatha-ignore no-eval -- why */' },
  {
    name: 'block-multiline',
    file: 'a.ts',
    source: '/*\n * debuggatha-ignore-next-line no-eval -- why\n */\neval(x);',
  },
  { name: 'in-string', file: 'a.ts', source: 'const s = "debuggatha-ignore no-eval";' },
  { name: 'em-dash', file: 'a.ts', source: 'x(); // debuggatha-ignore no-eval — emdash reason' },
  { name: 'empty-reason', file: 'a.ts', source: 'x(); // debuggatha-ignore no-eval -- ' },
  {
    name: 'reason-with-dashes',
    file: 'a.ts',
    source: 'x(); // debuggatha-ignore no-eval -- a -- b',
  },
  {
    name: 'unicode-reason',
    file: 'a.ts',
    source: 'x(); // debuggatha-ignore no-eval -- razón 日本語 🚀',
  },
  { name: 'nextline-typo', file: 'a.ts', source: '// debuggatha-ignore-nextline no-eval\nx();' },
  {
    name: 'two-directives',
    file: 'a.ts',
    source: '// debuggatha-ignore-file no-var\nx(); // debuggatha-ignore no-eval -- r\ny();',
  },
  { name: 'prefix-word', file: 'a.ts', source: '// notdebuggatha-ignore no-eval' },
  {
    name: 'crlf-line',
    file: 'a.ts',
    source: 'x(); // debuggatha-ignore no-eval -- why\r\ny();\r\n',
    note: 'ts-bug-crlf',
  },
  {
    name: 'crlf-block',
    file: 'a.ts',
    source: '/* debuggatha-ignore a\r\n b */ z',
    note: 'ts-bug-crlf',
  },
];

const rules = ['no-eval', 'no-var', 'eqeqeq', 'no-explicit-any', 'anything'];
const out = cases.map((c) => {
  const lexed = lex(c.source, c.file);
  const directives = parseDirectives(lexed.comments);
  const lines = c.source.split('\n').length + 1;
  const probes: [number, string, number][] = [];
  for (let line = 1; line <= lines + 1; line += 1) {
    for (const rule of rules) {
      const d = findDirective(directives, line, rule);
      probes.push([line, rule, d ? directives.indexOf(d) : -1]);
    }
  }
  return { ...c, comments: lexed.comments, directives, probes };
});

const files = [
  'a.ts',
  'a.py',
  'a.rb',
  'a.sh',
  'a.yml',
  'a.toml',
  'a.env',
  'a.html',
  'a.vue',
  'a.svelte',
  'a.md',
  'A.PY',
  'a.rs',
  'noext',
];
const comments = files.map((f) => [f, inlineSuppressionComment(f, 'no-eval', 'because')]);

writeFileSync(
  'crates/debuggatha-engine/tests/fixtures/suppress-conformance.json',
  `${JSON.stringify({ cases: out, comments }, null, 2)}\n`,
);
console.log(`${out.length} cases`);
