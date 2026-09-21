import { describe, expect, it } from "bun:test";
import { CATEGORIES, createFinding } from "@debuggatha/review-engine";
import { defaultFindingMatcher } from "./matching.js";
import { createLedgerEntry } from "./types/entry.js";
import { computeFindingFingerprint } from "./types/fingerprint.js";

const association = {
  createdBySessionId: "session-1",
  lastUpdatedBySessionId: "session-1",
  repositorySnapshot: { generatedAt: "2026-07-10T00:00:00.000Z", fingerprint: "abc123" },
  appliedPolicy: undefined,
  appliedPacks: [],
};

function makeFinding(overrides: Partial<Parameters<typeof createFinding>[0]> = {}) {
  return createFinding({
    title: "x",
    explanation: "x",
    severity: "low",
    confidence: "high",
    category: CATEGORIES.Reliability,
    locations: [{ file: "src/a.ts", lines: { start: 10, end: 10 } }],
    evidence: [{ kind: "review-pack-rule", packId: "rust-pack", ruleId: "no-unwrap" }],
    ...overrides,
  });
}

function entryFor(finding: ReturnType<typeof makeFinding>) {
  return createLedgerEntry({
    fingerprint: computeFindingFingerprint(finding),
    finding,
    review: association,
    origin: { kind: "review", sessionId: "session-1" },
  });
}

describe("defaultFindingMatcher", () => {
  it("reports 'new' when nothing in the active set shares file+rule", () => {
    const outcome = defaultFindingMatcher(makeFinding(), []);
    expect(outcome).toEqual({ kind: "new" });
  });

  it("matches the same file+rule even when the line number moved", () => {
    const original = makeFinding({
      locations: [{ file: "src/a.ts", lines: { start: 10, end: 10 } }],
    });
    const entry = entryFor(original);
    const movedDown = makeFinding({
      locations: [{ file: "src/a.ts", lines: { start: 55, end: 55 } }],
    });

    const outcome = defaultFindingMatcher(movedDown, [entry]);

    expect(outcome).toEqual({ kind: "matched", entryId: entry.id, changed: false });
  });

  it("does not match across different files even with the same rule", () => {
    const entry = entryFor(makeFinding({ locations: [{ file: "src/a.ts", lines: undefined }] }));
    const otherFile = makeFinding({ locations: [{ file: "src/b.ts", lines: undefined }] });

    expect(defaultFindingMatcher(otherFile, [entry])).toEqual({ kind: "new" });
  });

  it("does not match across different rules in the same file", () => {
    const entry = entryFor(
      makeFinding({
        evidence: [{ kind: "review-pack-rule", packId: "rust-pack", ruleId: "no-unwrap" }],
      }),
    );
    const differentRule = makeFinding({
      evidence: [{ kind: "review-pack-rule", packId: "rust-pack", ruleId: "no-panic" }],
    });

    expect(defaultFindingMatcher(differentRule, [entry])).toEqual({ kind: "new" });
  });

  it("falls back to (file, category) identity when neither finding cites a rule", () => {
    const noRule = makeFinding({
      category: CATEGORIES.Maintainability,
      evidence: [{ kind: "documentation", file: "README.md", excerpt: "x" }],
    });
    const entry = entryFor(noRule);
    const sameFileAndCategory = makeFinding({
      category: CATEGORIES.Maintainability,
      evidence: [{ kind: "documentation", file: "README.md", excerpt: "y" }],
    });

    expect(defaultFindingMatcher(sameFileAndCategory, [entry])).toEqual({
      kind: "matched",
      entryId: entry.id,
      changed: false,
    });
  });

  it("reports changed:true when both sides have a content anchor and it differs", () => {
    const before = makeFinding({
      evidence: [
        { kind: "code", file: "src/a.ts", lines: { start: 10, end: 10 }, excerpt: "x.unwrap()" },
        { kind: "review-pack-rule", packId: "rust-pack", ruleId: "no-unwrap" },
      ],
    });
    const entry = entryFor(before);
    const after = makeFinding({
      evidence: [
        { kind: "code", file: "src/a.ts", lines: { start: 12, end: 12 }, excerpt: "y.unwrap()" },
        { kind: "review-pack-rule", packId: "rust-pack", ruleId: "no-unwrap" },
      ],
    });

    expect(defaultFindingMatcher(after, [entry])).toEqual({
      kind: "matched",
      entryId: entry.id,
      changed: true,
    });
  });
});
