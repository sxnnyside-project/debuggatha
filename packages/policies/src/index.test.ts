import type { RepositoryContext } from "@debuggatha/repository-intelligence";
import { createReviewRequest } from "@debuggatha/review-engine";
import { describe, expect, it } from "vitest";
import { assemblePolicy, defaultCapabilityRegistry } from "./index.js";

function fixtureContext(): RepositoryContext {
  return {
    rootDir: "/repo",
    generatedAt: "2026-07-10T00:00:00.000Z",
    hasGit: true,
    stack: {
      languages: [{ value: "TypeScript", evidence: [] }],
      frameworks: [],
      buildSystems: [],
      packageManagers: [],
      runtimes: [],
      platformTargets: [],
      workspaceType: "single-package",
      workspaceEvidence: [],
      manifests: [],
    },
    capabilities: [
      { id: "typescript", kind: "language", confidence: "high", evidence: [], origin: "manifest" },
    ],
    dependencies: { lockfiles: [], declaredVersions: {}, runtimeConstraints: {} },
    documentation: { sources: [], summary: "" },
    criteria: { rules: [], sources: [] },
    understanding: { confidence: "high", openQuestions: [] },
  };
}

describe("assemblePolicy", () => {
  it("resolves a ReviewPolicy against the pre-populated review-packs registry", () => {
    const context = fixtureContext();
    const request = createReviewRequest({
      scope: { kind: "workspace" },
      depth: "full",
      requestedPackIds: ["debuggatha/typescript"],
    });

    const policy = assemblePolicy(context, request);

    expect(policy.rules.map((r) => r.id)).toContain("no-explicit-any");
    expect(policy.packRefs).toEqual([{ id: "debuggatha/typescript", version: "1.0.0" }]);
  });

  it("exposes the registry review-packs was wired against", () => {
    expect(defaultCapabilityRegistry.getPack("debuggatha/typescript")?.id).toBe(
      "debuggatha/typescript",
    );
  });

  it("fails closed on a pack id nothing in review-packs registers", () => {
    const request = createReviewRequest({
      scope: { kind: "workspace" },
      depth: "full",
      requestedPackIds: ["does-not-exist"],
    });
    expect(() => assemblePolicy(fixtureContext(), request)).toThrow(/does-not-exist/);
  });
});
