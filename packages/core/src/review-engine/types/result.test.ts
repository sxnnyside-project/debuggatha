import { afterEach, describe, expect, it } from "bun:test";
import { buildRepositoryContext } from "../../repository-intelligence/index.js";
import { withTempRepo } from "../../testing/index.js";
import { CATEGORIES } from "./category.js";
import { createFinding } from "./finding.js";
import { createReviewRequest } from "./request.js";
import { createReviewResult } from "./result.js";
import { createReviewSession, transitionSession } from "./session.js";

function completedSessionFixture() {
  const repo = withTempRepo({ "README.md": "# Demo\n" });
  const repositoryContext = buildRepositoryContext(repo.root);
  const request = createReviewRequest({ scope: { kind: "workspace" }, depth: "full" });
  const requested = createReviewSession({
    request,
    repositoryContext,
    selectedPacks: [{ id: "react-pack", version: "1.0.0" }],
    selectedPolicy: { id: "policy-1" },
  });
  const prepared = transitionSession(requested, "prepared");
  const running = transitionSession(prepared, "running", {
    source: { kind: "deterministic-analyzer", name: "debuggatha-core" },
  });
  const completed = transitionSession(running, "completed", { durationMs: 500 });
  return { completed, cleanup: repo.cleanup };
}

describe("createReviewResult", () => {
  let cleanup: (() => void) | undefined;

  afterEach(() => {
    cleanup?.();
    cleanup = undefined;
  });

  it("builds a result tied to the session by id, with a computed summary", () => {
    const fixture = completedSessionFixture();
    cleanup = fixture.cleanup;

    const finding = createFinding({
      title: "Unused variable",
      explanation: "Never read after assignment.",
      severity: "low",
      confidence: "high",
      category: CATEGORIES.Maintainability,
      locations: [{ file: "src/index.ts", lines: undefined }],
      evidence: [{ kind: "language-convention", language: "TypeScript", detail: "no-unused-vars" }],
    });

    const result = createReviewResult({ session: fixture.completed, findings: [finding] });

    expect(result.sessionId).toBe(fixture.completed.id);
    expect(result.summary.totalFindings).toBe(1);
    expect(result.summary.appliedPacks).toEqual([{ id: "react-pack", version: "1.0.0" }]);
    expect(result.summary.appliedPolicies).toEqual([{ id: "policy-1" }]);
    expect(result.summary.durationMs).toBe(500);
  });

  it("refuses to build a result from a session that never completed", () => {
    const repo = withTempRepo({ "README.md": "# Demo\n" });
    cleanup = repo.cleanup;
    const repositoryContext = buildRepositoryContext(repo.root);
    const request = createReviewRequest({ scope: { kind: "workspace" }, depth: "quick" });
    const requested = createReviewSession({ request, repositoryContext });

    expect(() => createReviewResult({ session: requested, findings: [] })).toThrow(
      /only a "completed" session produces a result/,
    );
  });
});
