import { afterAll, describe, expect, it } from 'vitest';
import type { RepositoryContext, ReviewPolicy } from '../contracts/index.js';
import { removeRepos } from '../test/fixtures.js';
import { IntelligenceProvider } from './intelligenceProvider.js';

afterAll(removeRepos);

/** What the daemon's `repository/context` and `repository/policy` answer for a small repository. */
function loaded() {
  const context = {
    rootDir: '/w',
    stack: {
      languages: [{ value: 'typescript' }],
      frameworks: [{ value: 'react' }],
      buildSystems: [{ value: 'bun' }],
    },
    documentation: { sources: [{ kind: 'readme', file: 'README.md' }] },
    criteria: { rules: [{ id: 'strict-types', description: 'Types are strict.' }] },
  } as unknown as RepositoryContext;
  const policy: ReviewPolicy = { id: 'p-1', packRefs: [{ id: 'typescript', version: '1.0.0' }] };
  const provider = new IntelligenceProvider();
  provider.refresh(context, policy);
  return { provider, context, policy };
}

describe('IntelligenceProvider', () => {
  it('shows nothing until a review has built a context', async () => {
    expect(await new IntelligenceProvider().getChildren()).toEqual([]);
  });

  it('offers the four sections of what Debuggatha understood', async () => {
    const { provider } = loaded();
    const roots = await provider.getChildren();
    expect(roots.map((node) => node.label)).toEqual([
      'Stack',
      'Documentation',
      'Criteria',
      'Active Review Policy',
    ]);
  });

  it('lists the detected stack under Stack', async () => {
    const { provider, context } = loaded();
    const [stack] = await provider.getChildren();
    const labels = (await provider.getChildren(stack)).map((node) => node.label);
    expect(labels.length).toBe(
      context.stack.languages.length +
        context.stack.frameworks.length +
        context.stack.buildSystems.length,
    );
    expect(labels.length).toBeGreaterThan(0);
  });

  it('lists every pack the policy resolved with its version', async () => {
    const { provider, policy } = loaded();
    const roots = await provider.getChildren();
    const policyNode = roots.find((node) => node.nodeId === 'policy');
    const packs = await provider.getChildren(policyNode);
    expect(packs.map((node) => node.label)).toEqual(policy.packRefs.map((pack) => pack.id));
    expect(packs.every((node) => node.description)).toBe(true);
  });

  it('returns nothing beneath a leaf and notifies listeners on refresh', async () => {
    const { provider, context, policy } = loaded();
    let fired = 0;
    provider.onDidChangeTreeData(() => {
      fired += 1;
    });
    provider.refresh(context, policy);
    expect(fired).toBe(1);
    const [stack] = await provider.getChildren();
    const [leaf] = await provider.getChildren(stack);
    expect(await provider.getChildren(leaf)).toEqual([]);
  });
});
