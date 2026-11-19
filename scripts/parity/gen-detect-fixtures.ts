// Writes the differential fixtures for the Rust detector runner and the finding excerpt by running
// the TypeScript engine (the parity reference, ADR-0006). Run: bun scripts/parity/gen-detect-fixtures.ts
//
// 1. Excerpts: the real `no-var` detector over lines of every awkward shape; its hit carries
//    `excerptOf(line)`, the text the ledger fingerprint is made from.
// 2. Runner: a toy detector registered under real and fake rule ids, run through
//    `runReviewSkillsEngine` over a hand-made policy, so the finding the engine builds (title,
//    evidence order, recommendation, edit, severity and category fallbacks, suppression) is compared.
import { writeFileSync } from 'node:fs';
import type { ReviewPolicy } from '../../packages/core/src/index.js';
import {
  type DetectorHit,
  type DetectorInput,
  RULE_DETECTORS,
} from '../../packages/engine/src/skills/engine/detectors.js';
import { runReviewSkillsEngine } from '../../packages/engine/src/skills/engine/run.js';

const wellFormed = (s: string) => s.toWellFormed();

// --- 1. excerpts -----------------------------------------------------------------------------
const tokens = {
  aws: 'AKIAABCDEFGHIJKLMNOP',
  gh: `ghp_${'a'.repeat(36)}`,
  ghpat: `github_pat_${'b'.repeat(50)}`,
  slack: 'xoxb-1234567890-abcdef',
  stripe: `sk_live_${'c'.repeat(16)}`,
  google: `AIza${'d'.repeat(35)}`,
  pem: '-----BEGIN RSA PRIVATE KEY-----',
  pem2: '-----BEGIN PRIVATE KEY-----',
  jwt: `eyJ${'e'.repeat(10)}.eyJ${'f'.repeat(10)}.${'g'.repeat(10)}`,
  sendgrid: `SG.${'h'.repeat(22)}.${'i'.repeat(43)}`,
};
const lines: string[] = [
  'var a = 1;',
  '   var a = 1;   ',
  '\tvar a = 1;\t',
  ' var a = 1; ',
  '﻿var a = 1;',
  'var a = 1;\r',
  'var a = "é ✓ 日本語";',
  'var a = "😀";',
  `var a = "${'x'.repeat(190)}";`,
  `var a = "${'x'.repeat(191)}";`,
  `var a = "${'x'.repeat(192)}😀";`,
  `var a = "${'x'.repeat(193)}😀";`,
  `var a = "${'x'.repeat(194)}😀😀";`,
  `var a = "${'日'.repeat(300)}";`,
  `var a = "${'é'.repeat(150)}";`,
  ...Object.values(tokens).map((t) => `var a = "${t}";`),
  `var a = "${tokens.aws} and ${tokens.gh} and ${tokens.aws}";`,
  `var a = "${'x'.repeat(170)}${tokens.stripe}";`,
  `var a = "${'x'.repeat(185)}${tokens.aws}";`,
  `var a = "${tokens.aws}${'y'.repeat(300)}";`,
  `var a = "AKIA${'Z'.repeat(15)}";`,
  `var a = "xAKIAABCDEFGHIJKLMNOP";`,
  `var a = "${tokens.jwt}.${tokens.jwt}";`,
  `var a = "[redacted] ${tokens.aws}";`,
];
const excerpts = lines.map((line) => {
  const input: DetectorInput = {
    file: 'a.js',
    content: line,
    lineNumbers: undefined,
    masked: line,
    strings: [],
  };
  const detector = RULE_DETECTORS['no-var'];
  const hit = detector?.(input)[0];
  return { line: wellFormed(line), excerpt: hit ? wellFormed(hit.excerpt) : null };
});

// --- 2. runner -------------------------------------------------------------------------------
const toy =
  (confidence: DetectorHit['confidence']) =>
  (input: DetectorInput): DetectorHit[] =>
    input.content.split('\n').flatMap((line, index) => {
      const at = line.indexOf('TOY');
      if (at < 0) return [];
      const hit: DetectorHit = {
        index,
        lines: { start: index + 1, end: index + 1 },
        columns: { start: at + 1, end: at + 4 },
        excerpt: line.trim(),
        confidence,
      };
      if (line.includes('SEV')) hit.severity = 'critical';
      if (line.includes('EDIT'))
        hit.edit = { startColumn: at + 1, endColumn: at + 4, replacement: 'OK', safety: 'safe' };
      return [hit];
    });
RULE_DETECTORS['toy-pack'] = toy('high');
RULE_DETECTORS['toy-criteria'] = toy('medium');
RULE_DETECTORS['toy-nocat'] = toy('low');
RULE_DETECTORS.eqeqeq = toy('high'); // a rule with real guidance
RULE_DETECTORS['no-var'] = toy('medium'); // a rule with real guidance and an `edit`

const pack = (id: string) => ({ kind: 'pack' as const, packId: 'p', packVersion: '1.0.0', id });
const policy = {
  id: 'pol1',
  packRefs: [{ id: 'p', version: '1.0.0' }],
  conflicts: [],
  rules: [
    {
      id: 'toy-pack',
      statement: 'First sentence. Second sentence.',
      category: 'security',
      defaultSeverity: 'high',
      origin: pack('x'),
    },
    {
      id: 'toy-criteria',
      statement: 'Line one\nline two',
      category: 'maintainability',
      defaultSeverity: 'low',
      origin: { kind: 'criteria' },
    },
    { id: 'toy-nocat', statement: '.leading dot statement', origin: pack('y') },
    {
      id: 'eqeqeq',
      statement: 'Use ===. Always.',
      category: 'correctness',
      defaultSeverity: 'medium',
      origin: pack('z'),
    },
    {
      id: 'no-var',
      statement: '  Use let  ',
      category: 'maintainability',
      defaultSeverity: 'low',
      origin: pack('w'),
    },
    {
      id: 'has-no-detector',
      statement: 'Never runs',
      category: 'x',
      defaultSeverity: 'high',
      origin: pack('v'),
    },
  ],
} as unknown as ReviewPolicy;

const units = [
  {
    file: 'src/a.ts',
    content:
      'a TOY b\nplain\n  TOY SEV EDIT here\n// debuggatha-ignore toy-pack -- accepted\nx TOY\n',
    lineNumbers: undefined,
  },
  {
    file: 'src/b.py',
    content:
      "# nothing\nx = 'TOY é ✓ 日本語'\n# debuggatha-ignore-next-line toy-criteria, eqeqeq\ny = 'TOY next'\nz = 'TOY after'\n",
    lineNumbers: undefined,
  },
  {
    file: 'src/c.ts',
    content:
      "/* debuggatha-ignore-file no-var -- vendored */\nconst a = 'TOY 1';\nconst b = 'TOY 2';\n",
    lineNumbers: undefined,
  },
];
const suppressed: unknown[] = [];
const findings = runReviewSkillsEngine(policy, units, { onSuppressed: (s) => suppressed.push(s) });

// Ids are random: renumber in order of first appearance.
const ids = new Map<string, string>();
const normalized = JSON.parse(JSON.stringify(findings)).map((f: { id: string }) => {
  ids.set(f.id, `n${ids.size + 1}`);
  return { ...f, id: ids.get(f.id) };
});

writeFileSync(
  'crates/debuggatha-engine/tests/fixtures/detect-conformance.json',
  `${JSON.stringify({ excerpts, policy, units, findings: normalized, suppressed }, null, 2)}\n`,
);
console.log(
  `${excerpts.length} excerpts, ${normalized.length} findings, ${suppressed.length} suppressed`,
);
