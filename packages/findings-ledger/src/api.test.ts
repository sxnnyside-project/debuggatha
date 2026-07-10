import { CATEGORIES, createFinding } from "@debuggatha/review-engine";
import { describe, expect, it } from "vitest";
import { getEntry, getHistory, listEntries, updateFindingStatus } from "./api.js";
import { createLedgerEntry } from "./types/entry.js";
import { computeFindingFingerprint } from "./types/fingerprint.js";
import { createEmptyLedger } from "./types/ledger.js";

const association = {
  createdBySessionId: "s1",
  lastUpdatedBySessionId: "s1",
  repositorySnapshot: { generatedAt: "2026-07-10T00:00:00.000Z", fingerprint: "abc" },
  appliedPolicy: undefined,
  appliedPacks: [],
};

function ledgerWithOneEntry(
  overrides: { severity?: "critical" | "high" | "medium" | "low"; file?: string } = {},
) {
  const finding = createFinding({
    title: "x",
    explanation: "x",
    severity: overrides.severity ?? "high",
    confidence: "high",
    category: CATEGORIES.Security,
    locations: [{ file: overrides.file ?? "a.ts", lines: undefined }],
    evidence: [{ kind: "language-convention", language: "TypeScript", detail: "x" }],
  });
  const entry = createLedgerEntry({
    fingerprint: computeFindingFingerprint(finding),
    finding,
    review: association,
    origin: { kind: "review", sessionId: "s1" },
  });
  return { ledger: { ...createEmptyLedger("/repo"), entries: [entry] }, entry };
}

describe("listEntries", () => {
  it("filters by status, severity, category, and file, combined with AND semantics", () => {
    const { ledger, entry } = ledgerWithOneEntry({ severity: "high", file: "a.ts" });

    expect(listEntries(ledger, { status: ["open"] })).toEqual([entry]);
    expect(listEntries(ledger, { status: ["resolved"] })).toEqual([]);
    expect(listEntries(ledger, { severity: ["high"], file: "a.ts" })).toEqual([entry]);
    expect(listEntries(ledger, { severity: ["low"] })).toEqual([]);
  });

  it("returns everything when no filter is given", () => {
    const { ledger } = ledgerWithOneEntry();
    expect(listEntries(ledger)).toHaveLength(1);
  });
});

describe("getEntry / getHistory", () => {
  it("finds an entry by id and returns its history", () => {
    const { ledger, entry } = ledgerWithOneEntry();

    expect(getEntry(ledger, entry.id)).toEqual(entry);
    expect(getHistory(ledger, entry.id)).toEqual(entry.history);
  });

  it("throws reading history for an unknown entry id, rather than returning an empty array", () => {
    const { ledger } = ledgerWithOneEntry();
    expect(() => getHistory(ledger, "does-not-exist")).toThrow(/No ledger entry/);
  });
});

describe("updateFindingStatus", () => {
  it("is the only way to change a status — transitions the named entry and returns a new frozen ledger", () => {
    const { ledger, entry } = ledgerWithOneEntry();

    const updated = updateFindingStatus(ledger, entry.id, "acknowledged", {
      kind: "manual",
      actor: "jane",
    });

    expect(ledger.entries[0]?.status).toBe("open");
    expect(getEntry(updated, entry.id)?.status).toBe("acknowledged");
    expect(Object.isFrozen(updated)).toBe(true);
  });

  it("throws for an unknown entry id", () => {
    const { ledger } = ledgerWithOneEntry();
    expect(() =>
      updateFindingStatus(ledger, "does-not-exist", "acknowledged", {
        kind: "manual",
        actor: "jane",
      }),
    ).toThrow(/No ledger entry/);
  });

  it("propagates an illegal-transition error instead of silently no-op'ing", () => {
    const { ledger, entry } = ledgerWithOneEntry();
    expect(() =>
      updateFindingStatus(ledger, entry.id, "reopened", { kind: "manual", actor: "jane" }),
    ).toThrow(/Illegal finding lifecycle transition/);
  });
});
