// Writes the differential fixtures for the Rust review request, lifecycle and session by running
// the TypeScript implementation (the parity reference, ADR-0006).
// Run: bun scripts/parity/gen-session-fixtures.ts
//
// Ids and timestamps are given explicitly, so nothing is random. The context is built from a small
// repository and its root path and build time are normalized to "." and "T".
import { writeFileSync } from 'node:fs';
import {
  buildRepositoryContext,
  createReviewRequest,
  createReviewSession,
  REVIEW_LIFECYCLE_STATUSES,
  type ReviewLifecycleStatus,
  type ReviewSession,
  transitionSession,
} from '../../packages/core/src/index.js';
import { withTempRepo } from '../../packages/core/src/testing/index.js';

const repo = withTempRepo({
  'README.md': '# Demo\n\nA demo.\n',
  'package.json': JSON.stringify({ name: 'demo', dependencies: { react: '18' } }),
});
const context = JSON.parse(JSON.stringify(buildRepositoryContext(repo.root)));
repo.cleanup();
context.rootDir = '.';
context.generatedAt = 'T';

const T0 = '2026-07-10T00:00:00.000Z';
const source = { kind: 'deterministic-analyzer', name: 'debuggatha-core' } as const;
type Patch = Parameters<typeof transitionSession>[2];

const requests = [
  { name: 'diff-with-base', input: { scope: { kind: 'diff', base: 'main' }, depth: 'full' } },
  { name: 'diff-no-base', input: { scope: { kind: 'diff', base: undefined }, depth: 'quick' } },
  { name: 'files', input: { scope: { kind: 'files', paths: ['a.ts', 'b.ts'] }, depth: 'quick' } },
  {
    name: 'workspace-with-packs',
    input: {
      scope: { kind: 'workspace' },
      depth: 'architectural',
      requestedPackIds: ['react', 'owasp'],
      requestedPolicyId: 'abc123',
    },
  },
  { name: 'files-empty', input: { scope: { kind: 'files', paths: [] }, depth: 'quick' } },
].map(({ name, input }) => {
  try {
    return {
      name,
      input,
      request: createReviewRequest({ id: 'r1', requestedAt: T0, ...input } as never),
    };
  } catch (e) {
    return { name, input, error: (e as Error).message };
  }
});

const okRequest = requests.find((r) => r.name === 'workspace-with-packs')?.request;
if (!okRequest) throw new Error('fixture request missing');
const base = createReviewSession({
  id: 's1',
  createdAt: T0,
  request: okRequest,
  repositoryContext: context,
  selectedPolicy: { id: 'abc123' },
  selectedPacks: [{ id: 'react', version: '1.0.0' }],
});

const walks: { name: string; steps: { to: ReviewLifecycleStatus; patch?: Patch }[] }[] = [
  {
    name: 'happy-path',
    steps: [
      { to: 'prepared' },
      { to: 'running', patch: { source, startedAt: T0 } },
      { to: 'completed', patch: { completedAt: '2026-07-10T00:00:01.000Z', durationMs: 1000 } },
    ],
  },
  {
    name: 'fails-with-error',
    steps: [{ to: 'prepared' }, { to: 'failed', patch: { error: 'boom — é ✓' } }],
  },
  { name: 'cancel-early', steps: [{ to: 'cancelled' }] },
  { name: 'skip-state', steps: [{ to: 'running' }] },
  { name: 'out-of-terminal', steps: [{ to: 'cancelled' }, { to: 'requested' }] },
];

// Every pair of statuses, from a session forced into each state, so the table is checked whole.
const allowed: Record<string, string[]> = {};
for (const from of REVIEW_LIFECYCLE_STATUSES) {
  allowed[from] = [];
  for (const to of REVIEW_LIFECYCLE_STATUSES) {
    try {
      transitionSession({ ...base, status: from } as ReviewSession, to);
      allowed[from]?.push(to);
    } catch {
      // illegal, as expected
    }
  }
}

const sessions = walks.map(({ name, steps }) => {
  let session = base;
  const trail: string[] = [];
  for (const step of steps) {
    try {
      session = transitionSession(session, step.to, step.patch);
      trail.push(session.status);
    } catch (e) {
      return {
        name,
        steps,
        trail,
        error: (e as Error).message,
        final: JSON.parse(JSON.stringify(session)),
      };
    }
  }
  return { name, steps, trail, final: JSON.parse(JSON.stringify(session)) };
});

writeFileSync(
  'crates/debuggatha-core/tests/fixtures/session-conformance.json',
  `${JSON.stringify(
    { context, requests, session: JSON.parse(JSON.stringify(base)), sessions, allowed },
    null,
    2,
  )}\n`,
);
console.log(`requests ${requests.length}, walks ${sessions.length}`);
