// Writes the differential fixtures for the Rust syntax layer by running the TypeScript line lexer
// (`lex.ts`, the parity reference, ADR-0006) over small JavaScript and TypeScript sources.
// Run: bun scripts/parity/gen-syntax-fixtures.ts
//
// The Rust layer parses instead of lexing, so it is allowed to differ where the lexer is wrong:
// each case says whether the two must agree ("same") or how they differ and why ("differs").
import { writeFileSync } from 'node:fs';
import { lex } from '../../packages/engine/src/skills/engine/lex.js';

type Case = {
  name: string;
  file: string;
  source: string;
  expect: 'same' | 'differs';
  why?: string;
};

const cases: Case[] = [
  { name: 'plain-code', file: 'a.ts', source: 'const a = 1;\nexport default a;\n', expect: 'same' },
  { name: 'line-comment', file: 'a.ts', source: 'const a = 1; // process.env.X\n', expect: 'same' },
  {
    name: 'block-comment-multiline',
    file: 'a.ts',
    source: '/* process.env.X\n  still */ const a = 1;\n',
    expect: 'same',
  },
  { name: 'double-quoted', file: 'a.ts', source: 'const s = "process.env.X";\n', expect: 'same' },
  {
    name: 'single-quoted-escape',
    file: 'a.ts',
    source: "const s = 'it\\'s process.env.X';\n",
    expect: 'same',
  },
  {
    name: 'template-no-expr',
    file: 'a.ts',
    source: 'const s = `process.env.X`;\n',
    expect: 'same',
  },
  {
    name: 'template-with-expr',
    file: 'a.ts',
    source: 'const s = `a ${process.env.X} b`;\n',
    expect: 'differs',
    why: 'the expression inside ${} is code; the lexer hides the whole template',
  },
  {
    name: 'regex-with-quote',
    file: 'a.ts',
    source: 'const r = /[\'"]/g; const k = process.env.K;\n',
    expect: 'same',
  },
  {
    name: 'jsx-text-apostrophe',
    file: 'a.tsx',
    source: "export const A = () => <p>don't process.env.X</p>;\nconst k = process.env.K;\n",
    expect: 'differs',
    why: 'JSX text is not a string, but an apostrophe in it opens one for the lexer',
  },
  {
    name: 'division-not-regex',
    file: 'a.ts',
    source: 'const q = a / b; const k = process.env.K; const z = c / d;\n',
    expect: 'same',
  },
  {
    name: 'unicode-before-string',
    file: 'a.ts',
    source: 'const é = "日本語 process.env.X"; // 🚀 x\nconst k = 1;\n',
    expect: 'same',
  },
  {
    name: 'nested-template',
    file: 'a.ts',
    source: 'const s = `a ${`b ${process.env.X}`} c`;\n',
    expect: 'differs',
    why: 'nested template expressions are code',
  },
  {
    name: 'crlf',
    file: 'a.ts',
    source: 'const a = "x";\r\n// c\r\nconst b = 2;\r\n',
    expect: 'same',
  },
  {
    name: 'directive-and-import',
    file: 'a.ts',
    source: '"use strict";\nimport x from "pkg/process.env";\n',
    expect: 'same',
  },
];

const out = cases.map((c) => {
  const r = lex(c.source, c.file);
  return { ...c, tsMasked: r.masked, tsComments: r.comments.length, tsStrings: r.strings.length };
});
writeFileSync(
  'crates/debuggatha-engine/tests/fixtures/syntax-conformance.json',
  `${JSON.stringify({ cases: out }, null, 2)}\n`,
);
console.log(`${out.length} cases`);
