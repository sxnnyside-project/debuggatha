import { describe, expect, it } from "bun:test";
import { CATEGORIES, createFinding } from "@debuggatha/review-engine";
import { createLedgerEntry, transitionFinding } from "./entry.js";
import { computeFindingFingerprint } from "./fingerprint.js";
import { createEmptyLedger } from "./ledger.js";
import { summarizeLedger } from "./summary.js";

const association = {
  createdBySessionId: "s1",
  lastUpdatedBySessionId: "s1",
  repositorySnapshot: { generatedAt: "2026-07-10T00:00:00.000Z", fingerprint: "abc" },
  appliedPolicy: undefined,
  appliedPacks: [],
};

function entryWith(severity: "critical" | "high" | "medium" | "low", category: string) {
  const finding = createFinding({
    title: "x",
    explanation: "x",
    severity,
    confidence: "high",
    category,
    locations: [{ file: "a.ts", lines: undefined }],
    evidence: [{ kind: "language-convention", language: "TypeScript", detail: "x" }],
  });
  return createLedgerEntry({
    fingerprint: computeFindingFingerprint(finding),
    finding,
    review: association,
    origin: { kind: "review", sessionId: "s1" },
  });
}

describe("summarizeLedger", () => {
  it("counts every entry by status, including resolved and dismissed", () => {
    const open = entryWith("high", CATEGORIES.Security);
    const dismissed = transitionFinding(entryWith("low", CATEGORIES.Maintainability), "dismissed", {
      kind: "manual",
      actor: "jane",
    });
    const ledger = { ...createEmptyLedger("/repo"), entries: [open, dismissed] };

    const summary = summarizeLedger(ledger);

    expect(summary.countsByStatus.open).toBe(1);
    expect(summary.countsByStatus.dismissed).toBe(1);
    expect(summary.countsByStatus.resolved).toBe(0);
  });

  it("only counts active (open/acknowledged/reopened) entries in the severity/category breakdown", () => {
    const active = entryWith("critical", CATEGORIES.Security);
    const dismissed = transitionFinding(entryWith("critical", CATEGORIES.Security), "dismissed", {
      kind: "manual",
      actor: "jane",
    });
    const ledger = { ...createEmptyLedger("/repo"), entries: [active, dismissed] };

    const summary = summarizeLedger(ledger);

    expect(summary.activeBySeverity.critical).toBe(1);
    expect(summary.activeByCategory[CATEGORIES.Security]).toBe(1);
  });
});
