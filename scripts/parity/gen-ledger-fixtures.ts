// Writes the differential fixtures for the Rust findings ledger by running the TypeScript
// ledger (the parity reference, ADR-0006) over real findings and non-ASCII excerpts.
//
//   bun scripts/parity/gen-ledger-fixtures.ts
//
// Ids and timestamps are random in the TypeScript engine, so the fixtures record a projection
// in which ids are renumbered (n1, n2, ...) in order of first appearance and timestamps are left out.
import { readFileSync, writeFileSync } from 'node:fs';
import { canonicalStringify } from '../../packages/core/src/findings-ledger/internal/hash.js';
import {
  associationFor,
  buildRepositoryContext,
  computeFindingFingerprint,
  createEmptyLedger,
  deserializeLedger,
  type Finding,
  type Ledger,
  type LedgerEntry,
  type ReviewResult,
  serializeLedger,
  snapshotRefFor,
  summarizeLedger,
  synchronizeReviewResult,
  updateFindingStatus,
} from '../../packages/core/src/index.js';

const DIR = 'crates/debuggatha-core/tests/fixtures';
const real: Finding[] = JSON.parse(readFileSync(`${DIR}/real-findings.json`, 'utf8')).findings;

const context = {
  generatedAt: '2026-01-01T00:00:00.000Z',
  stack: [],
  dependencies: [],
  documentation: [],
  criteria: [],
} as never;

function resultOf(sessionId: string, findings: Finding[]): ReviewResult {
  return {
    sessionId,
    summary: {
      totalFindings: findings.length,
      findingsBySeverity: { critical: 0, high: 0, medium: 0, low: 0, informational: 0 },
      findingsByCategory: {},
      durationMs: undefined,
      appliedPacks: [{ id: 'pack-a', version: '1.0.0' }],
      appliedPolicies: [{ id: 'policy-1' }],
    },
    findings,
    recommendations: [],
    execution: {
      source: undefined,
      startedAt: undefined,
      completedAt: undefined,
      durationMs: undefined,
      error: undefined,
    },
  };
}

function withExcerpt(finding: Finding, excerpt: string): Finding {
  return {
    ...finding,
    evidence: finding.evidence.map((e) => (e.kind === 'code' ? { ...e, excerpt } : e)),
  };
}

function finding(id: string, file: string, evidence: Finding['evidence'], category = 'security') {
  return {
    id,
    title: id,
    explanation: 'x',
    severity: 'medium',
    confidence: 'high',
    category,
    locations: [{ file, lines: { start: 1, end: 1 } }],
    evidence,
    appliedPolicy: undefined,
    originatingPack: undefined,
    recommendations: [],
  } as Finding;
}

type Action =
  | { type: 'sync'; sessionId: string; findings: Finding[]; scope: unknown }
  | {
      type: 'update';
      entryIndex: number;
      to: string;
      origin: { kind: 'manual'; actor: string };
      comment?: string;
    };

const firstFile = real[0]?.locations[0]?.file ?? '';
const mutated = real.slice(10, 20).map((f) => withExcerpt(f, 'changed excerpt é ✓'));
const scenarios: { name: string; actions: Action[] }[] = [
  {
    name: 'real-findings',
    actions: [
      { type: 'sync', sessionId: 's1', findings: real, scope: { kind: 'workspace' } },
      {
        type: 'sync',
        sessionId: 's2',
        findings: [...mutated, ...real.slice(20)],
        scope: { kind: 'workspace' },
      },
      {
        type: 'update',
        entryIndex: 10,
        to: 'dismissed',
        origin: { kind: 'manual', actor: 'person' },
        comment: 'wontfix — é ✓ 🚀',
      },
      { type: 'sync', sessionId: 's3', findings: real, scope: { kind: 'workspace' } },
      { type: 'update', entryIndex: 0, to: 'acknowledged', origin: { kind: 'manual', actor: 'p' } },
      {
        type: 'sync',
        sessionId: 's4',
        findings: [],
        scope: { kind: 'files', files: [firstFile] },
      },
      { type: 'update', entryIndex: 5, to: 'resolved', origin: { kind: 'manual', actor: 'p' } },
      { type: 'update', entryIndex: 5, to: 'acknowledged', origin: { kind: 'manual', actor: 'p' } },
    ],
  },
  {
    name: 'rule-less-and-non-ascii',
    actions: [
      {
        type: 'sync',
        sessionId: 'a1',
        scope: { kind: 'workspace' },
        findings: [
          finding(
            'r1',
            'src/a.ts',
            [{ kind: 'framework-convention', framework: 'react', detail: 'd' }],
            'maintainability',
          ),
          finding('r2', 'src/a.ts', [
            {
              kind: 'code',
              file: 'src/a.ts',
              lines: undefined,
              excerpt: 'const é = "日本語"; // 🚀',
            },
          ]),
          finding('r3', 'src/b.ts', [
            {
              kind: 'external-analyzer',
              tool: 'biome',
              ruleId: 'lint/x',
              version: undefined,
              license: 'MIT',
              url: undefined,
            },
          ]),
          finding('r4', 'src/b.ts', [
            {
              kind: 'semantic-review',
              role: 'detection',
              provider: 'ollama',
              model: undefined,
              verdict: undefined,
              reason: 'r',
              topic: 'race',
            },
          ]),
          finding('r5', 'src/c.ts', [{ kind: 'criteria', ruleId: 'crit-1', source: 'CLAUDE.md' }]),
        ],
      },
      {
        type: 'sync',
        sessionId: 'a2',
        scope: { kind: 'workspace' },
        findings: [
          finding(
            'r1b',
            'src/a.ts',
            [{ kind: 'framework-convention', framework: 'react', detail: 'other' }],
            'maintainability',
          ),
          finding('r2b', 'src/a.ts', [
            {
              kind: 'code',
              file: 'src/a.ts',
              lines: undefined,
              excerpt: 'const é = "日本語"; // 🚀 changed',
            },
          ]),
          finding('r3b', 'src/b.ts', [
            {
              kind: 'external-analyzer',
              tool: 'biome',
              ruleId: 'lint/x',
              version: '2',
              license: 'MIT',
              url: undefined,
            },
          ]),
        ],
      },
    ],
  },
];

const UUID = /[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}/g;

function project(ledger: Ledger) {
  return ledger.entries.map((e: LedgerEntry) => ({
    id: e.id,
    status: e.status,
    fingerprint: e.fingerprint,
    latestFindingId: e.latestFinding.id,
    review: {
      createdBySessionId: e.review.createdBySessionId,
      lastUpdatedBySessionId: e.review.lastUpdatedBySessionId,
    },
    history: e.history.map((h) => ({
      previousState: h.previousState,
      newState: h.newState,
      origin: h.origin,
      comment: h.comment,
    })),
  }));
}

const out = scenarios.map((scenario) => {
  let ledger = createEmptyLedger('/repo');
  const steps = scenario.actions.map((action) => {
    if (action.type === 'sync') {
      const result = resultOf(action.sessionId, action.findings);
      const association = associationFor(result, context);
      const synced = synchronizeReviewResult(ledger, result, context, action.scope as never);
      ledger = synced.ledger;
      return {
        action: {
          type: 'sync',
          sessionId: action.sessionId,
          findings: action.findings,
          scope: action.scope,
          association,
        },
        expected: {
          report: synced.report,
          entries: project(ledger),
          summary: summarizeLedger(ledger),
        },
      };
    }
    const id = ledger.entries[action.entryIndex]?.id as string;
    try {
      ledger = updateFindingStatus(ledger, id, action.to as never, action.origin, action.comment);
      return {
        action: { ...action, entryId: id },
        expected: { entries: project(ledger), summary: summarizeLedger(ledger) },
      };
    } catch (error) {
      return { action: { ...action, entryId: id }, expected: { error: (error as Error).message } };
    }
  });
  const serialized = serializeLedger(ledger).replace(
    /("(?:timestamp|createdAt|updatedAt)": ")[^"]*"/g,
    '$1T"',
  );
  return { name: scenario.name, steps, serialized };
});

// Fingerprints of a spread of excerpts, hashed the way Node hashes them (UTF-8).
const excerpts = [
  '',
  'plain ascii',
  'const é = 1;',
  '日本語のコメント',
  '🚀 astral plane',
  'á combining',
  'crlf\r\nline',
  '👨‍👩‍👧 zjw family',
  `${'x'.repeat(199)}😀`,
  'tab\tand "quotes" and \\ backslash',
  ' line separator ',
];
const fingerprints = excerpts.map((excerpt) => ({
  excerpt,
  fingerprint: computeFindingFingerprint(
    finding('fp', 'src/fp.ts', [{ kind: 'code', file: 'src/fp.ts', lines: undefined, excerpt }]),
  ),
}));

// Canonical JSON: key order by UTF-16 unit, number formatting, escapes.
const canonicalInputs = [
  JSON.stringify({ b: 1, a: 2 }),
  JSON.stringify({ z: [3, { y: 1, x: [] }], a: {} }),
  // U+1F600 (a surrogate pair) must sort before U+FFFF in UTF-16 order, after it in UTF-8 order.
  JSON.stringify({ '\u{1F600}': 1, '\uFFFF': 2, a: 3 }),
  JSON.stringify({ é: 1, e: 2, 日: 3, Z: 4 }),
  // Written as text because JavaScript literals would already round the large integers.
  '[0.5,1e21,1e-7,123456789012345680000,100,1.5e300,0.000001,-3,9007199254740993,18446744073709551615]',
  JSON.stringify('line\nbreak \u0001 \u007f \u2028 " \\ \u{1F600}'),
  '[null,true,false,""]',
  JSON.stringify({ k: null, deep: { b: { d: 1, c: 2 }, a: [{ z: 1, y: 2 }] } }),
];
const canonical = canonicalInputs.map((json) => ({
  json,
  expected: canonicalStringify(JSON.parse(json)),
}));

// Persistence: what the TypeScript reader refuses, and with which message.
const emptyLedger = JSON.parse(JSON.stringify(createEmptyLedger('/repo')));
const refusals = [
  { json: JSON.stringify({ entries: [] }), message: 'missing or non-numeric schemaVersion' },
  {
    json: JSON.stringify({ ...emptyLedger, schemaVersion: 2 }),
    message: 'schema version 2 is not supported',
  },
  {
    json: JSON.stringify({ ...emptyLedger, schemaVersion: 1.5 }),
    message: 'schema version 1.5 is not supported',
  },
  {
    json: JSON.stringify({ ...emptyLedger, schemaVersion: '1' }),
    message: 'missing or non-numeric schemaVersion',
  },
];
for (const r of refusals) {
  try {
    deserializeLedger(r.json);
    throw new Error('TypeScript accepted a ledger the fixtures say it refuses');
  } catch (error) {
    if (!(error as Error).message.includes(r.message)) throw error;
  }
}

// Snapshot hashes of real repository contexts (this checkout, two of its packages).
const snapshots = ['.', 'packages/cli', 'apps/vscode'].map((dir) => {
  const ctx = buildRepositoryContext(dir);
  const parts = JSON.parse(
    JSON.stringify({
      stack: ctx.stack,
      dependencies: ctx.dependencies,
      documentation: ctx.documentation,
      criteria: ctx.criteria,
    }),
  );
  return { dir, parts, expected: snapshotRefFor(ctx).fingerprint };
});

// Replace UUIDs by n1, n2... in order of first appearance across the whole document.
const seen = new Map<string, string>();
const text = JSON.stringify({
  fingerprints,
  scenarios: out,
  canonical,
  refusals,
  snapshots,
}).replace(UUID, (id) => {
  if (!seen.has(id)) seen.set(id, `n${seen.size + 1}`);
  return seen.get(id) as string;
});
writeFileSync(`${DIR}/ledger-conformance.json`, text);
console.log(`wrote ${DIR}/ledger-conformance.json: ${out.length} scenarios, ${seen.size} ids`);
