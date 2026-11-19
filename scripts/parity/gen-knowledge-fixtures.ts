// Writes the differential fixtures for the Rust knowledge system by running the TypeScript
// implementation (the parity reference, ADR-0006). Run: bun scripts/parity/gen-knowledge-fixtures.ts
//
// Everything here is deterministic: no ids or timestamps are produced by the code under test.
import { writeFileSync } from 'node:fs';
import {
  buildRepositoryContext,
  createInMemoryRegistry,
  type ReviewPack,
  resolvePolicy,
  type SkillDescriptor,
  validateReviewPack,
} from '../../packages/core/src/index.js';
import {
  compareVersions,
  satisfiesRange,
} from '../../packages/core/src/knowledge-system/internal/semver.js';
import { reviewPacks } from '../../packages/packs/src/index.js';

const OUT = 'crates/debuggatha-core/tests/fixtures/knowledge-conformance.json';
const clean = <T>(value: T): T => JSON.parse(JSON.stringify(value));

type Rule = ReviewPack['rules'][number];
const rule = (id: string, packId: string, over: Partial<Rule> = {}): Rule => ({
  id,
  packId,
  statement: `statement of ${id}`,
  category: 'maintainability',
  appliesTo: { kind: 'always' },
  defaultSeverity: 'medium',
  knowledgeRefs: ['k'],
  contradicts: undefined,
  ...over,
});
const knowledge = { id: 'k', title: 't', body: 'b', externalRefs: undefined, limitations: ['l'] };
const pack = (
  id: string,
  version: string,
  rules: Rule[],
  dependsOn: ReviewPack['dependsOn'] = [],
  kind: ReviewPack['kind'] = 'stack',
): ReviewPack => ({
  id,
  version,
  kind,
  displayName: id,
  rules,
  knowledge: [knowledge],
  dependsOn,
});

// ---- synthetic packs: conflicts, ranges, skills, awkward ids --------------------------------
const syntheticPacks: ReviewPack[] = [
  pack(
    'alpha',
    '1.0.0',
    [
      rule('shared-rule', 'alpha'),
      rule('Zeta', 'alpha'),
      rule('alpha-1', 'alpha', { contradicts: ['beta-1'] }),
      rule('é-rule', 'alpha'),
      rule('É-rule', 'alpha'),
      rule('react-only', 'alpha', {
        appliesTo: { kind: 'requires-framework', framework: 'React' },
      }),
      rule('next-only', 'alpha', {
        appliesTo: { kind: 'requires-framework', framework: 'Next.js' },
      }),
      rule('rust-only', 'alpha', { appliesTo: { kind: 'requires-language', language: 'Rust' } }),
      rule('glob-rule', 'alpha', { appliesTo: { kind: 'glob', pattern: '**/*.ts' } }),
      rule('a_b', 'alpha'),
      rule('a-b', 'alpha'),
      rule('a.b', 'alpha'),
      rule('10-items', 'alpha'),
      rule('9-items', 'alpha'),
    ],
    [{ packId: 'base', versionRange: '^1.0.0' }, { skillId: 'stack-detection' }],
  ),
  pack('beta', '2.1.0', [
    rule('shared-rule', 'beta', { statement: 'the beta wording' }),
    rule('beta-1', 'beta'),
    rule('mutual-1', 'beta', { contradicts: ['mutual-2'] }),
    rule('mutual-2', 'beta', { contradicts: ['mutual-1'] }),
  ]),
  pack('base', '1.0.0', [rule('base-old', 'base')]),
  pack('base', '1.2.3', [rule('base-mid', 'base')]),
  pack('base', '1.9.0', [rule('base-new', 'base'), rule('CRIT-clash', 'base')]),
  pack('base', '2.0.0', [rule('base-next-major', 'base')]),
  pack(
    'needs-missing',
    '1.0.0',
    [rule('nm-1', 'needs-missing')],
    [{ packId: 'ghost', versionRange: '^3.0.0' }, { skillId: 'no-such-skill' }],
  ),
  pack('cycle-a', '1.0.0', [rule('ca-1', 'cycle-a')], [{ packId: 'cycle-b', versionRange: '*' }]),
  pack('cycle-b', '1.0.0', [rule('cb-1', 'cycle-b')], [{ packId: 'cycle-a', versionRange: '*' }]),
  pack('concern-x', '0.1.0', [rule('cx-1', 'concern-x')], [], 'concern'),
];
const skills: SkillDescriptor[] = [
  { id: 'stack-detection', tier: 'core', description: 'd' },
  { id: 'file-review', tier: 'review', description: 'd' },
];

// ---- repository contexts: three real ones and two synthetic ----------------------------------
function realContext(dir: string) {
  const ctx = buildRepositoryContext(dir);
  return {
    name: `real:${dir}`,
    generatedAt: ctx.generatedAt,
    capabilityIds: ctx.capabilities.map((c) => c.id),
    criteria: clean(ctx.criteria),
    _ctx: ctx,
  };
}
const contexts = [
  ...['.', 'packages/cli', 'apps/vscode'].map(realContext),
  {
    name: 'synthetic:clashing-criteria',
    generatedAt: '2026-01-01T00:00:00.000Z',
    capabilityIds: ['react', 'nextjs', 'typescript'],
    criteria: {
      rules: [
        { id: 'shared-rule', description: 'the repository wins' },
        { id: 'CRIT-clash', description: 'criteria version' },
        { id: 'criteria:claude-md:10', description: 'ten' },
        { id: 'criteria:claude-md:3', description: 'three' },
        { id: 'Beta', description: 'upper' },
        { id: 'alpha', description: 'lower' },
      ],
    },
    _ctx: null,
  },
  {
    name: 'synthetic:no-criteria',
    generatedAt: '2026-01-01T00:00:00.000Z',
    capabilityIds: ['rust'],
    criteria: { rules: [] },
    _ctx: null,
  },
];
const asContext = (c: (typeof contexts)[number]) =>
  (c._ctx ?? {
    generatedAt: c.generatedAt,
    capabilities: c.capabilityIds.map((id) => ({ id })),
    criteria: c.criteria,
  }) as never;

// ---- policies --------------------------------------------------------------------------------
type Outcome = { policy: unknown } | { error: string };
const policies: unknown[] = [];
function policyCase(
  ctx: (typeof contexts)[number],
  set: 'real' | 'synthetic',
  requestedPackIds: string[],
  requestedPolicyId?: string,
) {
  const registry = createInMemoryRegistry(
    set === 'real' ? reviewPacks : syntheticPacks,
    set === 'real' ? [] : skills,
  );
  let expected: Outcome;
  try {
    expected = {
      policy: clean(
        resolvePolicy(asContext(ctx), { requestedPackIds, requestedPolicyId } as never, registry),
      ),
    };
  } catch (error) {
    expected = { error: (error as Error).message };
  }
  policies.push({ context: ctx.name, set, requestedPackIds, requestedPolicyId, expected });
  return expected;
}
const realIds = reviewPacks.map((p) => p.id);
for (const ctx of contexts.slice(0, 3)) {
  const all = policyCase(ctx, 'real', realIds);
  for (const id of realIds.slice(0, 6)) policyCase(ctx, 'real', [id]);
  policyCase(ctx, 'real', [...realIds].reverse());
  if ('policy' in all) {
    const id = (all.policy as { id: string }).id;
    policyCase(ctx, 'real', realIds, id); // pinned and still valid
    policyCase(ctx, 'real', realIds, 'deadbeefdeadbeef'); // pinned and stale
  }
}
policyCase(contexts[0] as never, 'real', ['no-such-pack']);
for (const ctx of contexts.slice(3)) {
  for (const ids of [
    ['alpha'],
    ['beta'],
    ['beta', 'alpha'],
    ['alpha', 'beta'],
    ['alpha', 'beta', 'concern-x'],
    ['needs-missing'],
    ['cycle-a'],
    ['base'],
    [],
  ]) {
    policyCase(ctx, 'synthetic', ids);
  }
}

// ---- registry --------------------------------------------------------------------------------
const registry = createInMemoryRegistry(syntheticPacks, skills);
const ranges = [
  undefined,
  '*',
  '',
  '^1.0.0',
  '^1.2.0',
  '~1.2.0',
  '~1.0.0',
  '1.2.3',
  '2.0.0',
  '^2.0.0',
  '^3.0.0',
  'junk',
  '^junk',
];
const getPack = ranges.map((range) => ({
  id: 'base',
  range: range ?? null,
  expected: registry.getPack('base', range)?.version ?? null,
}));
const resolveDeps = [
  ['alpha'],
  ['needs-missing'],
  ['cycle-a'],
  ['alpha', 'beta'],
  ['ghost'],
  ['beta', 'beta'],
].map((ids) => {
  const r = registry.resolveDependencies(ids);
  return {
    ids,
    resolved: r.resolved.map((p) => `${p.id}@${p.version}`),
    missing: clean(r.missing),
  };
});

// ---- semver ----------------------------------------------------------------------------------
const versions = [
  '1.2.3',
  '1.2.4',
  '1.10.0',
  '2.0.0',
  '0.0.1',
  '1.2',
  ' 1.2.3 ',
  'v1.2.3',
  '1.2.3-beta.1',
  '01.2.3',
  '1.2.3.4',
  '99999999999999999999.0.0',
  'x',
  '',
  '\u00851.2.3',
  '﻿1.2.3',
];
const rangeSet = [
  '*',
  '',
  '^1.2.3',
  '^1.0.0',
  '~1.2.0',
  '~1.2.3',
  '1.2.3',
  ' ^1.2.3 ',
  '^x',
  '~',
  'junk',
  '^0.0.1',
  '>=1.0.0',
];
const semver = {
  compare: versions.flatMap((a) => versions.map((b) => [a, b, Math.sign(compareVersions(a, b))])),
  satisfies: versions.flatMap((v) => rangeSet.map((r) => [v, r, satisfiesRange(v, r)])),
};

// ---- validation ------------------------------------------------------------------------------
const bad = (over: Partial<ReviewPack>, rules?: Rule[]): ReviewPack => ({
  ...pack('p', '1.0.0', rules ?? [rule('r', 'p')]),
  ...over,
});
const toValidate: ReviewPack[] = [
  ...reviewPacks,
  ...syntheticPacks,
  bad({ id: '  ' }),
  bad({ version: '1.2' }),
  bad({ version: 'v1.2.3' }),
  bad({ version: ' 1.2.3-rc ' }),
  bad({}, [rule('r', 'p'), rule('r', 'p')]),
  bad({}, [rule('r', 'other')]),
  bad({}, [rule('r', 'p', { knowledgeRefs: [] })]),
  bad({}, [rule('r', 'p', { knowledgeRefs: ['nope', 'k'] })]),
  bad({ knowledge: [knowledge, knowledge] }),
  bad({ knowledge: [{ ...knowledge, limitations: [] }] }),
  bad({ dependsOn: [{ packId: '', versionRange: '*' } as never] }),
  bad({ dependsOn: [{ packId: 'x', versionRange: '  ' }] }),
  bad({ dependsOn: [{ skillId: '' }] }),
  bad({ version: 'x', id: '', knowledge: [{ ...knowledge, limitations: [] }] }, [
    rule('r', 'zzz', { knowledgeRefs: [] }),
  ]),
];
const validate = toValidate.map((p) => {
  try {
    validateReviewPack(p);
    return { pack: clean(p), expected: null };
  } catch (error) {
    return { pack: clean(p), expected: (error as Error).message };
  }
});

// ---- collation -------------------------------------------------------------------------------
const words = [
  'a',
  'B',
  'b',
  'A',
  'a-b',
  'ab',
  'a_b',
  'a.b',
  'a:b',
  'a1',
  'a-1',
  '10',
  '9',
  'é',
  'e',
  'E',
  'É',
  'f',
  'ß',
  'ss',
  '-',
  '_',
  ' ',
  'x y',
  'xy',
  'Z',
  'z',
  'ä',
  'ä',
  'ø',
  'o',
  'ñ',
  'n',
  'ç',
  'c',
  'Æ',
  'ae',
  '日本',
  '日',
  '中',
  '😀',
  'no-eval',
  'no-explicit-any',
  'no-EVAL',
  'No-Eval',
  'criteria:a:10',
  'criteria:a:3',
  'ǆ',
  'dž',
  'ﬁ',
  'fi',
  'İ',
  'i',
  'I',
  'ı',
  '',
  '​',
  'a​b',
  'ab\u0000',
];
const collation = {
  words,
  sorted: [...words].sort((a, b) => a.localeCompare(b)),
  pairs: words.flatMap((a) => words.map((b) => [a, b, Math.sign(a.localeCompare(b))])),
};

const document = {
  realPacks: clean(reviewPacks),
  syntheticPacks: clean(syntheticPacks),
  skills,
  contexts: contexts.map(({ _ctx, ...rest }) => rest),
  policies,
  registry: { getPack, resolveDeps },
  semver,
  validate,
  collation,
};
writeFileSync(OUT, JSON.stringify(document));
const errors = policies.filter((p) => 'error' in (p as { expected: object }).expected).length;
console.log(
  `wrote ${OUT}: ${reviewPacks.length} real packs, ${policies.length} policies (${errors} errors), ${validate.length} validations, ${collation.pairs.length} collation pairs`,
);
