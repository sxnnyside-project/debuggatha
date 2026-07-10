import { buildRepositoryContext } from "@debuggatha/repository-intelligence";
import { withTempRepo } from "@debuggatha/testing";
import { afterEach, describe, expect, it } from "vitest";
import { associationFor, snapshotRefFor } from "./association.js";

describe("snapshotRefFor", () => {
  let cleanup: (() => void) | undefined;

  afterEach(() => {
    cleanup?.();
    cleanup = undefined;
  });

  it("produces the same fingerprint for the same repository content", () => {
    const repo = withTempRepo({ "README.md": "# Demo\n" });
    cleanup = repo.cleanup;
    const context = buildRepositoryContext(repo.root);

    expect(snapshotRefFor(context)).toEqual(snapshotRefFor(context));
  });

  it("produces a different fingerprint when the repository's criteria differ", () => {
    const withCriteria = withTempRepo({
      "README.md": "# Demo\n",
      ".eslintrc.json": JSON.stringify({ rules: { "no-console": "warn" } }),
    });
    const withoutCriteria = withTempRepo({ "README.md": "# Demo\n" });

    const a = snapshotRefFor(buildRepositoryContext(withCriteria.root));
    const b = snapshotRefFor(buildRepositoryContext(withoutCriteria.root));
    withCriteria.cleanup();
    withoutCriteria.cleanup();

    expect(a.fingerprint).not.toBe(b.fingerprint);
  });
});

describe("associationFor", () => {
  let cleanup: (() => void) | undefined;

  afterEach(() => {
    cleanup?.();
    cleanup = undefined;
  });

  it("sets createdBySessionId and lastUpdatedBySessionId to the same session on creation", () => {
    const repo = withTempRepo({ "README.md": "# Demo\n" });
    cleanup = repo.cleanup;
    const context = buildRepositoryContext(repo.root);

    const association = associationFor(
      {
        sessionId: "session-1",
        summary: {
          totalFindings: 0,
          findingsBySeverity: { critical: 0, high: 0, medium: 0, low: 0, informational: 0 },
          findingsByCategory: {},
          durationMs: undefined,
          appliedPacks: [{ id: "react-pack", version: "1.0.0" }],
          appliedPolicies: [{ id: "policy-1" }],
        },
        findings: [],
        recommendations: [],
        execution: {
          source: undefined,
          startedAt: undefined,
          completedAt: undefined,
          durationMs: undefined,
          error: undefined,
        },
      },
      context,
    );

    expect(association.createdBySessionId).toBe("session-1");
    expect(association.lastUpdatedBySessionId).toBe("session-1");
    expect(association.appliedPolicy).toEqual({ id: "policy-1" });
    expect(association.appliedPacks).toEqual([{ id: "react-pack", version: "1.0.0" }]);
  });
});
