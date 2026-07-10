import type { RepositoryContext } from "@debuggatha/repository-intelligence";
import type { KnowledgeEntry, ReviewPack, Rule } from "../index.js";

/**
 * Test-only fixture builders — hand-constructed `RepositoryContext`/
 * `ReviewPack`/`Rule` values instead of running a real filesystem scan, so
 * this package's tests never need `@debuggatha/repository-intelligence`'s
 * scanners or real pack content (ADR-0001 consequence: "knowledge-system
 * can be fully unit-tested... without importing any real pack content").
 * Not exported from `src/index.ts` — internal to this package's test suite.
 */

export function fixtureRepositoryContext(
  overrides: Partial<RepositoryContext> = {},
): RepositoryContext {
  return {
    rootDir: "/repo",
    generatedAt: "2026-07-10T00:00:00.000Z",
    hasGit: true,
    stack: {
      languages: [{ value: "TypeScript", evidence: [] }],
      frameworks: [{ value: "React", evidence: [] }],
      buildSystems: [],
      packageManagers: [],
      runtimes: [],
      platformTargets: ["web"],
      workspaceType: "single-package",
      workspaceEvidence: [],
      manifests: ["package.json"],
    },
    dependencies: { lockfiles: [], declaredVersions: {}, runtimeConstraints: {} },
    documentation: { sources: [], summary: "" },
    criteria: { rules: [], sources: [] },
    understanding: { confidence: "high", openQuestions: [] },
    ...overrides,
  };
}

export function fixtureKnowledgeEntry(overrides: Partial<KnowledgeEntry> = {}): KnowledgeEntry {
  return {
    id: "knowledge-1",
    title: "Test knowledge entry",
    body: "The underlying domain expertise a rule derives from.",
    externalRefs: undefined,
    ...overrides,
  };
}

export function fixtureRule(overrides: Partial<Rule> = {}): Rule {
  return {
    id: "test-rule",
    packId: "test-pack",
    statement: "A test rule statement.",
    category: "maintainability",
    appliesTo: { kind: "always" },
    defaultSeverity: "medium",
    knowledgeRefs: ["knowledge-1"],
    contradicts: undefined,
    ...overrides,
  };
}

export function fixturePack(overrides: Partial<ReviewPack> = {}): ReviewPack {
  return {
    id: "test-pack",
    version: "1.0.0",
    kind: "stack",
    displayName: "Test Pack",
    rules: [fixtureRule()],
    knowledge: [fixtureKnowledgeEntry()],
    dependsOn: [],
    ...overrides,
  };
}
