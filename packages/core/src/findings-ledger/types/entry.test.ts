import { describe, expect, it } from "bun:test";
import { CATEGORIES, createFinding } from "../../review-engine/index.js";
import { createLedgerEntry, refreshLedgerEntry, transitionFinding } from "./entry.js";
import { computeFindingFingerprint } from "./fingerprint.js";

const finding = createFinding({
  title: "Unused variable",
  explanation: "Never read after assignment.",
  severity: "low",
  confidence: "high",
  category: CATEGORIES.Maintainability,
  locations: [{ file: "src/a.ts", lines: undefined }],
  evidence: [{ kind: "language-convention", language: "TypeScript", detail: "no-unused-vars" }],
});

const association = {
  createdBySessionId: "session-1",
  lastUpdatedBySessionId: "session-1",
  repositorySnapshot: { generatedAt: "2026-07-10T00:00:00.000Z", fingerprint: "abc123" },
  appliedPolicy: undefined,
  appliedPacks: [],
};

describe("createLedgerEntry", () => {
  it("starts open with a single 'created' history event", () => {
    const entry = createLedgerEntry({
      fingerprint: computeFindingFingerprint(finding),
      finding,
      review: association,
      origin: { kind: "review", sessionId: "session-1" },
    });

    expect(entry.status).toBe("open");
    expect(entry.history).toHaveLength(1);
    expect(entry.history[0]).toMatchObject({ previousState: undefined, newState: "open" });
    expect(entry.id).toBeTruthy();
    expect(entry.id).not.toBe(finding.id);
  });
});

describe("transitionFinding", () => {
  it("appends a history event and never mutates the original entry", () => {
    const entry = createLedgerEntry({
      fingerprint: computeFindingFingerprint(finding),
      finding,
      review: association,
      origin: { kind: "review", sessionId: "session-1" },
    });

    const acknowledged = transitionFinding(entry, "acknowledged", {
      kind: "manual",
      actor: "jane",
    });

    expect(entry.status).toBe("open");
    expect(entry.history).toHaveLength(1);
    expect(acknowledged.status).toBe("acknowledged");
    expect(acknowledged.history).toHaveLength(2);
    expect(acknowledged.history[1]).toMatchObject({
      previousState: "open",
      newState: "acknowledged",
      origin: { kind: "manual", actor: "jane" },
    });
  });

  it("preserves every prior history event when transitioning repeatedly", () => {
    let entry = createLedgerEntry({
      fingerprint: computeFindingFingerprint(finding),
      finding,
      review: association,
      origin: { kind: "review", sessionId: "session-1" },
    });
    entry = transitionFinding(entry, "acknowledged", { kind: "manual", actor: "jane" });
    entry = transitionFinding(entry, "resolved", { kind: "manual", actor: "jane" }, "fixed in #42");
    entry = transitionFinding(entry, "reopened", { kind: "review", sessionId: "session-2" });

    expect(entry.history.map((e) => e.newState)).toEqual([
      "open",
      "acknowledged",
      "resolved",
      "reopened",
    ]);
    expect(entry.history[2]?.comment).toBe("fixed in #42");
  });

  it("throws on an illegal transition and leaves history untouched", () => {
    const entry = createLedgerEntry({
      fingerprint: computeFindingFingerprint(finding),
      finding,
      review: association,
      origin: { kind: "review", sessionId: "session-1" },
    });

    expect(() => transitionFinding(entry, "reopened", { kind: "manual", actor: "jane" })).toThrow(
      /Illegal finding lifecycle transition/,
    );
  });
});

describe("refreshLedgerEntry", () => {
  it("updates the latest finding and last-updated session without touching status or history", () => {
    const entry = createLedgerEntry({
      fingerprint: computeFindingFingerprint(finding),
      finding,
      review: association,
      origin: { kind: "review", sessionId: "session-1" },
    });

    const refreshed = refreshLedgerEntry(entry, finding, computeFindingFingerprint(finding), {
      ...association,
      lastUpdatedBySessionId: "session-2",
    });

    expect(refreshed.status).toBe("open");
    expect(refreshed.history).toHaveLength(1);
    expect(refreshed.review.lastUpdatedBySessionId).toBe("session-2");
    expect(refreshed.review.createdBySessionId).toBe("session-1");
  });
});
