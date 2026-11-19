// Writes the ledger goldens of the corpus (phase 2 gate): for every repository, the findings the
// TypeScript engine reports (corpus/golden/ts-findings/<name>.json.gz, ids renumbered) and what the
// TypeScript ledger does with them (corpus/golden/ts-ledger/<name>.json).
//
//   node scripts/parity/run-corpus.mjs <corpus-dir> <tmp-out>      # with DEBUGGATHA_FINDINGS_DIR=<findings-dir>
//   bun scripts/parity/gen-ledger-golden.ts <findings-dir>
//
// The Rust test (crates/debuggatha-core/tests/ledger_corpus_gate.rs) replays the same five steps
// from the committed findings and must reproduce every fingerprint and every step.
import { createHash } from 'node:crypto';
import { mkdirSync, readdirSync, readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { gzipSync } from 'node:zlib';
import { canonicalStringify } from '../../packages/core/src/findings-ledger/internal/hash.js';
import {
  associationFor,
  computeFindingFingerprint,
  createEmptyLedger,
  type Finding,
  type Ledger,
  type ReviewResult,
  serializeLedger,
  summarizeLedger,
  synchronizeReviewResult,
  updateFindingStatus,
} from '../../packages/core/src/index.js';

const dir = process.argv[2];
if (!dir) {
  console.error('usage: bun scripts/parity/gen-ledger-golden.ts <findings-dir>');
  process.exit(1);
}
const FINDINGS_OUT = 'corpus/golden/ts-findings';
const LEDGER_OUT = 'corpus/golden/ts-ledger';
mkdirSync(FINDINGS_OUT, { recursive: true });
mkdirSync(LEDGER_OUT, { recursive: true });

const UUID = /[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}/g;
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

// The same edit the Rust test applies: every code excerpt gets a line added.
const edit = (finding: Finding): Finding => ({
  ...finding,
  evidence: finding.evidence.map((e) =>
    e.kind === 'code' && e.excerpt !== undefined
      ? { ...e, excerpt: `${e.excerpt}\n// edited é` }
      : e,
  ),
});

for (const file of readdirSync(dir).filter((f) => f.endsWith('.json'))) {
  const name = file.replace(/\.json$/, '');
  // Finding ids are random UUIDs; renumber them so the committed files are stable.
  const ids = new Map<string, string>();
  const text = readFileSync(join(dir, file), 'utf8').replace(UUID, (id) => {
    if (!ids.has(id)) ids.set(id, `f${ids.size + 1}`);
    return ids.get(id) as string;
  });
  const findings: Finding[] = JSON.parse(text);
  writeFileSync(join(FINDINGS_OUT, `${name}.json.gz`), gzipSync(text, { level: 9 }));

  // Entry ids are random too: renumber them n1, n2... in order of first appearance across steps.
  const entryIds = new Map<string, string>();
  const digest = (value: unknown) => {
    const canonical = canonicalStringify(value).replace(UUID, (id) => {
      if (!entryIds.has(id)) entryIds.set(id, `n${entryIds.size + 1}`);
      return entryIds.get(id) as string;
    });
    return createHash('sha256').update(canonical).digest('hex');
  };
  const project = (ledger: Ledger) =>
    ledger.entries.map((e) => ({
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

  let ledger = createEmptyLedger('/repo');
  const steps: unknown[] = [];
  const sync = (label: string, sessionId: string, list: Finding[], scope: never) => {
    const result = resultOf(sessionId, list);
    associationFor(result, context);
    const synced = synchronizeReviewResult(ledger, result, context, scope);
    ledger = synced.ledger;
    const report = Object.fromEntries(
      Object.entries(synced.report).map(([k, v]) => [k, (v as string[]).length]),
    );
    steps.push({
      step: label,
      report,
      entries: ledger.entries.length,
      digest: digest({
        entries: project(ledger),
        report: synced.report,
        summary: summarizeLedger(ledger),
      }),
    });
  };

  sync('first review', 's1', findings, { kind: 'workspace' } as never);
  sync(
    'fifth removed, seventh edited',
    's2',
    findings.flatMap((f, i) => (i % 5 === 0 ? [] : [i % 7 === 3 ? edit(f) : f])),
    { kind: 'workspace' } as never,
  );
  const open = ledger.entries.find((e) => e.status === 'open');
  if (open) {
    ledger = updateFindingStatus(
      ledger,
      open.id,
      'dismissed',
      { kind: 'manual', actor: 'person' },
      'wontfix é',
    );
    steps.push({
      step: 'dismiss first open',
      entries: ledger.entries.length,
      digest: digest({ entries: project(ledger), summary: summarizeLedger(ledger) }),
    });
  } else {
    steps.push({ step: 'dismiss first open', skipped: true });
  }
  sync('everything back', 's3', findings, { kind: 'workspace' } as never);
  const first = findings[0]?.locations[0]?.file;
  if (first !== undefined) {
    sync('nothing left in the first file', 's4', [], { kind: 'files', files: [first] } as never);
  } else {
    steps.push({ step: 'nothing left in the first file', skipped: true });
  }

  const serialized = serializeLedger(ledger).replace(
    /("(?:timestamp|createdAt|updatedAt)": ")[^"]*"/g,
    '$1T"',
  );
  const golden = {
    name,
    findings: findings.length,
    fingerprints: findings.map(computeFindingFingerprint),
    steps,
    finalLedgerDigest: digest(serialized),
  };
  writeFileSync(join(LEDGER_OUT, `${name}.json`), `${JSON.stringify(golden)}\n`);
}
console.log(`wrote ${FINDINGS_OUT} and ${LEDGER_OUT}`);
