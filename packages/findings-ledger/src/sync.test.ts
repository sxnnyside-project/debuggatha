import { buildRepositoryContext } from "@debuggatha/repository-intelligence";
import {
  CATEGORIES,
  createFinding,
  type Finding,
  type ReviewResult,
} from "@debuggatha/review-engine";
import { withTempRepo } from "@debuggatha/testing";
import { afterEach, describe, expect, it } from "vitest";
import { synchronizeReviewResult } from "./sync.js";
import { createEmptyLedger } from "./types/ledger.js";

function resultOf(sessionId: string, findings: Finding[]): ReviewResult {
  return {
    sessionId,
    summary: {
      totalFindings: findings.length,
      findingsBySeverity: { critical: 0, high: 0, medium: 0, low: 0, informational: 0 },
      findingsByCategory: {},
      durationMs: undefined,
      appliedPacks: [],
      appliedPolicies: [],
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

function findingIn(file: string, ruleId: string) {
  return createFinding({
    title: "x",
    explanation: "x",
    severity: "medium",
    confidence: "high",
    category: CATEGORIES.Reliability,
    locations: [{ file, lines: undefined }],
    evidence: [{ kind: "review-pack-rule", packId: "pack", ruleId }],
  });
}

describe("synchronizeReviewResult", () => {
  let cleanup: (() => void) | undefined;

  afterEach(() => {
    cleanup?.();
    cleanup = undefined;
  });

  function fixtureContext() {
    const repo = withTempRepo({ "README.md": "# Demo\n" });
    cleanup = repo.cleanup;
    return buildRepositoryContext(repo.root);
  }

  it("creates a new entry for a finding never seen before", () => {
    const context = fixtureContext();
    const ledger = createEmptyLedger(context.rootDir);

    const { ledger: next, report } = synchronizeReviewResult(
      ledger,
      resultOf("s1", [findingIn("a.ts", "r1")]),
      context,
      { kind: "workspace" },
    );

    expect(report.newEntryIds).toHaveLength(1);
    expect(next.entries).toHaveLength(1);
    expect(next.entries[0]?.status).toBe("open");
  });

  it("leaves an unchanged, re-detected finding open with no new history event", () => {
    const context = fixtureContext();
    const first = synchronizeReviewResult(
      createEmptyLedger(context.rootDir),
      resultOf("s1", [findingIn("a.ts", "r1")]),
      context,
      { kind: "workspace" },
    );

    const second = synchronizeReviewResult(
      first.ledger,
      resultOf("s2", [findingIn("a.ts", "r1")]),
      context,
      { kind: "workspace" },
    );

    expect(second.report.unchangedEntryIds).toHaveLength(1);
    expect(second.ledger.entries).toHaveLength(1);
    expect(second.ledger.entries[0]?.status).toBe("open");
    expect(second.ledger.entries[0]?.history).toHaveLength(1);
    expect(second.ledger.entries[0]?.review.createdBySessionId).toBe("s1");
    expect(second.ledger.entries[0]?.review.lastUpdatedBySessionId).toBe("s2");
  });

  it("auto-resolves an open finding that a workspace review no longer detects", () => {
    const context = fixtureContext();
    const first = synchronizeReviewResult(
      createEmptyLedger(context.rootDir),
      resultOf("s1", [findingIn("a.ts", "r1")]),
      context,
      { kind: "workspace" },
    );

    const second = synchronizeReviewResult(first.ledger, resultOf("s2", []), context, {
      kind: "workspace",
    });

    expect(second.report.autoResolvedEntryIds).toHaveLength(1);
    expect(second.ledger.entries[0]?.status).toBe("resolved");
    expect(second.ledger.entries[0]?.history.map((e) => e.newState)).toEqual(["open", "resolved"]);
  });

  it("never resolves a finding in a file outside this review's scope", () => {
    const context = fixtureContext();
    const first = synchronizeReviewResult(
      createEmptyLedger(context.rootDir),
      resultOf("s1", [findingIn("a.ts", "r1"), findingIn("b.ts", "r2")]),
      context,
      { kind: "workspace" },
    );

    // A file-scoped review of only a.ts must not resolve b.ts's finding,
    // even though b.ts's finding wasn't re-detected this run.
    const second = synchronizeReviewResult(
      first.ledger,
      resultOf("s2", [findingIn("a.ts", "r1")]),
      context,
      { kind: "files", files: ["a.ts"] },
    );

    expect(second.report.autoResolvedEntryIds).toHaveLength(0);
    const bEntry = second.ledger.entries.find((e) => e.fingerprint.file === "b.ts");
    expect(bEntry?.status).toBe("open");
  });

  it("reopens a resolved finding that reappears, recording it distinctly from a fresh 'new' finding", () => {
    const context = fixtureContext();
    const created = synchronizeReviewResult(
      createEmptyLedger(context.rootDir),
      resultOf("s1", [findingIn("a.ts", "r1")]),
      context,
      { kind: "workspace" },
    );
    const resolved = synchronizeReviewResult(created.ledger, resultOf("s2", []), context, {
      kind: "workspace",
    });
    expect(resolved.ledger.entries[0]?.status).toBe("resolved");

    const reappeared = synchronizeReviewResult(
      resolved.ledger,
      resultOf("s3", [findingIn("a.ts", "r1")]),
      context,
      { kind: "workspace" },
    );

    expect(reappeared.report.reopenedEntryIds).toHaveLength(1);
    expect(reappeared.report.newEntryIds).toHaveLength(0);
    expect(reappeared.ledger.entries).toHaveLength(1);
    expect(reappeared.ledger.entries[0]?.status).toBe("reopened");
    expect(reappeared.ledger.entries[0]?.history.map((e) => e.newState)).toEqual([
      "open",
      "resolved",
      "reopened",
    ]);
  });

  it("reports changed:true via the report when a matched finding's content anchor differs", () => {
    const context = fixtureContext();
    const withCode = (excerpt: string) =>
      createFinding({
        title: "x",
        explanation: "x",
        severity: "medium",
        confidence: "high",
        category: CATEGORIES.Reliability,
        locations: [{ file: "a.ts", lines: undefined }],
        evidence: [
          { kind: "review-pack-rule", packId: "pack", ruleId: "r1" },
          { kind: "code", file: "a.ts", lines: undefined, excerpt },
        ],
      });

    const first = synchronizeReviewResult(
      createEmptyLedger(context.rootDir),
      resultOf("s1", [withCode("before")]),
      context,
      { kind: "workspace" },
    );
    const second = synchronizeReviewResult(
      first.ledger,
      resultOf("s2", [withCode("after")]),
      context,
      { kind: "workspace" },
    );

    expect(second.report.changedEntryIds).toHaveLength(1);
    expect(second.report.unchangedEntryIds).toHaveLength(0);
  });

  it("is pure — the input ledger is untouched and every returned ledger is frozen", () => {
    const context = fixtureContext();
    const ledger = createEmptyLedger(context.rootDir);

    const { ledger: next } = synchronizeReviewResult(
      ledger,
      resultOf("s1", [findingIn("a.ts", "r1")]),
      context,
      { kind: "workspace" },
    );

    expect(ledger.entries).toHaveLength(0);
    expect(Object.isFrozen(next)).toBe(true);
  });
});
