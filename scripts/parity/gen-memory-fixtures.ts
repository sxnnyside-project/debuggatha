// Writes the differential fixtures for the Rust repository memory by running the TypeScript
// implementation (the parity reference, ADR-0006). Run: bun scripts/parity/gen-memory-fixtures.ts
//
// Ids and timestamps are random in the TypeScript engine: ids are renumbered m1, m2, ... in order
// of first appearance and timestamps are not projected (the text form maps them to "T").
import { readFileSync, writeFileSync } from 'node:fs';
import {
  activateMemory,
  addMemoryItem,
  archiveMemory,
  computeFindingFingerprint,
  confirmMemory,
  createEmptyMemoryStore,
  createMemoryItem,
  deprecateMemory,
  type Finding,
  filterSuppressedFindings,
  type MemoryItem,
  type MemoryStore,
  reactivateMemory,
  rejectMemory,
  removeMemoryItem,
  serializeMemoryStore,
  suggestMemory,
} from '../../packages/core/src/index.js';

const DIR = 'crates/debuggatha-core/tests/fixtures';
const real: Finding[] = JSON.parse(readFileSync(`${DIR}/real-findings.json`, 'utf8')).findings;
const fp = (i: number) => computeFindingFingerprint(real[i] as Finding);
const ruleless = {
  ...(real[0] as Finding),
  id: 'ruleless',
  category: 'maintainability',
  locations: [{ file: 'src/legacy.ts', lines: undefined }],
  evidence: [
    {
      kind: 'framework-convention',
      framework: 'react',
      detail: 'd',
    } as Finding['evidence'][number],
  ],
};
const rulelessFp = computeFindingFingerprint(ruleless);

const base = {
  confidence: 'documented' as const,
  rationale: 'because — é ✓ 日本語 🚀',
  evidence: [{ file: 'docs/ADR.md', detail: 'see the "decision" section' }],
  origin: { kind: 'user' as const, actor: 'someone' },
};

const inputs = [
  {
    ...base,
    category: 'suppression' as const,
    fingerprintFile: fp(0).file,
    fingerprintRuleId: fp(0).ruleId,
    fingerprintCategory: fp(0).category,
  },
  {
    ...base,
    category: 'suppression' as const,
    fingerprintFile: rulelessFp.file,
    fingerprintRuleId: undefined,
    fingerprintCategory: rulelessFp.category,
  },
  {
    ...base,
    category: 'accepted-deviation' as const,
    ruleId: fp(3).ruleId as string,
    scopeFile: undefined,
  },
  {
    ...base,
    category: 'accepted-deviation' as const,
    ruleId: fp(6).ruleId as string,
    scopeFile: 'no/such/file.ts',
  },
  {
    ...base,
    category: 'exception' as const,
    exceptionKind: 'layer-crossing' as const,
    ruleId: fp(9).ruleId,
    scopeFile: fp(9).file,
  },
  {
    ...base,
    category: 'exception' as const,
    exceptionKind: 'other' as const,
    ruleId: undefined,
    scopeFile: fp(12).file,
  },
  { ...base, category: 'convention' as const, rule: 'tests live beside the source' },
  {
    ...base,
    category: 'review-history' as const,
    ledgerEntryId: 'entry-1',
    note: 'reopened twice',
  },
  {
    ...base,
    category: 'decision' as const,
    decision: 'no ORM',
    documentationRef: 'docs/adr/0001.md',
  },
  { ...base, category: 'decision' as const, decision: 'monorepo', documentationRef: undefined },
];

type Fn =
  | 'suggest'
  | 'confirm'
  | 'activate'
  | 'reject'
  | 'deprecate'
  | 'reactivate'
  | 'archive'
  | 'remove';
type Action =
  | { type: 'create'; input: (typeof inputs)[number] }
  | { type: 'call'; fn: Fn; index: number; comment?: string }
  | { type: 'filter'; findings: Finding[] };

const actions: Action[] = inputs.map((input) => ({ type: 'create', input }));
const lifecycle = (index: number, upTo: Fn[]): Action[] =>
  upTo.map((fn) => ({ type: 'call', fn, index, comment: `note for ${fn} é` }));
const everyFinding = [...real.slice(0, 20), ruleless];
actions.push(
  { type: 'filter', findings: everyFinding }, // nothing is active yet: nothing is filtered
  ...lifecycle(0, ['suggest', 'confirm']),
  { type: 'filter', findings: everyFinding }, // confirmed is still not active
  ...lifecycle(0, ['activate']),
  ...lifecycle(1, ['suggest', 'confirm', 'activate']),
  ...lifecycle(2, ['suggest', 'confirm', 'activate']),
  ...lifecycle(3, ['suggest', 'confirm', 'activate']),
  ...lifecycle(4, ['suggest', 'confirm', 'activate']),
  ...lifecycle(5, ['suggest', 'confirm', 'activate']),
  { type: 'filter', findings: everyFinding },
  ...lifecycle(2, ['deprecate']),
  { type: 'filter', findings: everyFinding }, // a deprecated item filters nothing
  ...lifecycle(2, ['reactivate', 'archive']),
  { type: 'call', fn: 'activate', index: 2 }, // illegal: archived is terminal
  { type: 'call', fn: 'confirm', index: 6 }, // illegal: detected -> confirmed
  ...lifecycle(7, ['suggest', 'reject']),
  { type: 'call', fn: 'remove', index: 9 },
  { type: 'call', fn: 'suggest', index: 99 }, // unknown item
  { type: 'filter', findings: everyFinding },
);

function call(store: MemoryStore, a: Extract<Action, { type: 'call' }>): MemoryStore {
  const id = store.items[a.index]?.id ?? 'missing-id';
  switch (a.fn) {
    case 'suggest':
      return suggestMemory(store, id, 'session-1');
    case 'confirm':
      return confirmMemory(store, id, 'a', a.comment);
    case 'activate':
      return activateMemory(store, id, 'a');
    case 'reject':
      return rejectMemory(store, id, 'a', a.comment);
    case 'deprecate':
      return deprecateMemory(store, id, 'a', a.comment);
    case 'reactivate':
      return reactivateMemory(store, id, 'a');
    case 'archive':
      return archiveMemory(store, id, 'a', a.comment);
    case 'remove':
      return removeMemoryItem(store, id);
  }
}

function project(item: MemoryItem) {
  const { createdAt: _c, updatedAt: _u, history, ...rest } = item;
  return {
    ...rest,
    history: history.map(({ timestamp: _t, ...h }) => h),
  };
}

let store = createEmptyMemoryStore('/repo');
const steps = actions.map((action) => {
  if (action.type === 'create') {
    store = addMemoryItem(store, createMemoryItem(action.input as never));
    return { action, expected: { items: store.items.map(project) } };
  }
  if (action.type === 'filter') {
    const out = filterSuppressedFindings(action.findings, store);
    return {
      action: { type: 'filter', findings: action.findings },
      expected: {
        kept: out.findings.map((f) => f.id),
        suppressed: out.suppressed.map((s) => ({
          findingId: s.finding.id,
          memoryItemId: s.memoryItemId,
          reason: s.reason,
        })),
      },
    };
  }
  const itemId = store.items[action.index]?.id ?? 'missing-id';
  try {
    store = call(store, action);
    return { action: { ...action, itemId }, expected: { items: store.items.map(project) } };
  } catch (error) {
    return { action: { ...action, itemId }, expected: { error: (error as Error).message } };
  }
});

const serialized = serializeMemoryStore(store).replace(
  /("(?:timestamp|createdAt|updatedAt)": ")[^"]*"/g,
  '$1T"',
);

const refusals = [
  { json: JSON.stringify({ items: [] }), message: 'missing or non-numeric schemaVersion' },
  { json: JSON.stringify({ schemaVersion: 2 }), message: 'schema version 2 is not supported' },
];

const ids = new Map<string, string>();
const text = JSON.stringify({ steps, serialized, refusals }).replace(
  /[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}/g,
  (id) => {
    if (!ids.has(id)) ids.set(id, `m${ids.size + 1}`);
    return ids.get(id) as string;
  },
);
writeFileSync(`${DIR}/memory-conformance.json`, text);
console.log(`wrote ${DIR}/memory-conformance.json: ${steps.length} steps, ${ids.size} ids`);
