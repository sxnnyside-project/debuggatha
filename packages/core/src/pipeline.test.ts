import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, describe, expect, it, vi } from "vitest";
import * as real from "./index.js";
import { executeReview } from "./pipeline.js";

let dir: string | undefined;

afterEach(() => {
  if (dir) rmSync(dir, { recursive: true, force: true });
  dir = undefined;
});

function mktemp(): string {
  return mkdtempSync(join(tmpdir(), "debuggatha-pipeline-"));
}

describe("executeReview", () => {
  it("runs a real files review end-to-end and persists the ledger", () => {
    dir = mktemp();
    writeFileSync(join(dir, "package.json"), JSON.stringify({ name: "fixture" }));
    const target = join(dir, "config.js");
    // "release"'s `no-hardcoded-secrets` is `appliesTo: { kind: "always" }` —
    // unlike the stack packs (javascript/typescript), it doesn't depend on
    // Stack Detection emitting a language string the pack's `RuleScope`
    // exactly matches (see this package's README "Known limitations": stack
    // detection currently emits one combined "JavaScript/TypeScript" signal,
    // which never case-insensitively equals either stack pack's own
    // "javascript"/"typescript" `requires-language` scope).
    writeFileSync(target, 'const apiKey = "sk_live_1234567890abcdef";\n');

    const { result, syncReport } = executeReview({
      rootDir: dir,
      scope: { kind: "files", paths: [target] },
      depth: "full",
      packIds: ["debuggatha/release"],
      policyId: undefined,
      sourceName: "test",
    });

    expect(result.findings.length).toBeGreaterThan(0);
    expect(result.findings.some((f) => f.locations[0]?.file === target)).toBe(true);
    expect(syncReport.newEntryIds.length).toBe(result.findings.length);

    const ledgerPath = join(dir, ".debuggatha", "ledger.json");
    const persisted = JSON.parse(readFileSync(ledgerPath, "utf8"));
    expect(persisted.entries.length).toBe(result.findings.length);
  });

  it("runs a workspace review producing no findings for a clean, unmatched pack list", () => {
    dir = mktemp();
    writeFileSync(join(dir, "package.json"), JSON.stringify({ name: "fixture" }));
    mkdirSync(join(dir, "src"));
    writeFileSync(join(dir, "src", "clean.js"), "const x = 1;\n");

    const { result } = executeReview({
      rootDir: dir,
      scope: { kind: "workspace" },
      depth: "full",
      packIds: [],
      policyId: undefined,
      sourceName: "test",
    });

    expect(result.findings).toEqual([]);
  });

  it("uses injected deps instead of the real skill functions when provided", () => {
    dir = mktemp();
    writeFileSync(join(dir, "package.json"), JSON.stringify({ name: "fixture" }));

    const fakeReviewFiles = vi.fn().mockReturnValue([]);
    const { result } = executeReview(
      {
        rootDir: dir,
        scope: { kind: "files", paths: [join(dir, "package.json")] },
        depth: "full",
        packIds: [],
        policyId: undefined,
        sourceName: "test",
      },
      {
        buildRepositoryContext: real.buildRepositoryContext,
        createReviewRequest: real.createReviewRequest,
        assemblePolicy: real.assemblePolicy,
        createReviewSession: real.createReviewSession,
        transitionSession: real.transitionSession,
        createReviewResult: real.createReviewResult,
        reviewDiff: real.reviewDiff,
        reviewArchitecture: real.reviewArchitecture,
        reviewFiles: fakeReviewFiles,
        synchronizeReviewResult: real.synchronizeReviewResult,
        loadLedger: real.loadLedger,
        saveLedger: real.saveLedger,
        loadMemoryStore: real.loadMemoryStore,
        filterSuppressedFindings: real.filterSuppressedFindings,
      },
    );

    expect(fakeReviewFiles).toHaveBeenCalled();
    expect(result.findings).toEqual([]);
  });
});

describe("executeReview — Epic 12.5 capability fix", () => {
  it("resolves both javascript and typescript stack pack rules for a real TS repo (previously impossible)", () => {
    dir = mktemp();
    writeFileSync(
      join(dir, "package.json"),
      JSON.stringify({ name: "fixture", devDependencies: { typescript: "^5.0.0" } }),
    );
    writeFileSync(join(dir, "tsconfig.json"), "{}");
    const target = join(dir, "risky.ts");
    writeFileSync(target, "var x: any = eval(userInput);\n");

    const { result } = executeReview({
      rootDir: dir,
      scope: { kind: "files", paths: [target] },
      depth: "full",
      packIds: ["debuggatha/typescript", "debuggatha/javascript"],
      policyId: undefined,
      sourceName: "test",
    });

    const ruleIds = new Set(
      result.findings.flatMap((f) =>
        f.evidence.flatMap((e) => (e.kind === "review-pack-rule" ? [e.ruleId] : [])),
      ),
    );
    expect(ruleIds.has("no-explicit-any")).toBe(true);
  });
});

describe("executeReview — Epic 15 Repository Memory integration", () => {
  it("suppresses a finding matched by an active memory suppression, automatically, with no caller changes", () => {
    dir = mktemp();
    writeFileSync(join(dir, "package.json"), JSON.stringify({ name: "fixture" }));
    const target = join(dir, "config.js");
    writeFileSync(target, 'const apiKey = "sk_live_1234567890abcdef";\n');

    const before = executeReview({
      rootDir: dir,
      scope: { kind: "files", paths: [target] },
      depth: "full",
      packIds: ["debuggatha/release"],
      policyId: undefined,
      sourceName: "test",
    });
    expect(before.result.findings.length).toBeGreaterThan(0);
    expect(before.suppressedFindings).toEqual([]);

    const [firstFinding] = before.result.findings;
    if (!firstFinding) throw new Error("expected at least one finding");
    const fingerprint = real.computeFindingFingerprint(firstFinding);
    let store = real.createEmptyMemoryStore(dir);
    const item = real.createMemoryItem({
      category: "suppression",
      confidence: "user-confirmed",
      rationale: "Confirmed test fixture, not a real secret.",
      evidence: [{ file: target, detail: "reviewed by alice" }],
      fingerprintFile: fingerprint.file,
      fingerprintRuleId: fingerprint.ruleId,
      fingerprintCategory: fingerprint.category,
      origin: { kind: "user", actor: "alice" },
    });
    store = real.addMemoryItem(store, item);
    store = real.suggestMemory(store, item.id, "session-1");
    store = real.confirmMemory(store, item.id, "alice");
    store = real.activateMemory(store, item.id, "alice");
    real.saveMemoryStore(store);

    const after = executeReview({
      rootDir: dir,
      scope: { kind: "files", paths: [target] },
      depth: "full",
      packIds: ["debuggatha/release"],
      policyId: undefined,
      sourceName: "test",
    });

    expect(after.result.findings).toEqual([]);
    expect(after.suppressedFindings).toHaveLength(1);
    expect(after.suppressedFindings[0]).toMatchObject({
      memoryItemId: item.id,
      reason: "suppression",
    });
  });

  it("never suppresses using a merely-detected (unconfirmed) memory item", () => {
    dir = mktemp();
    writeFileSync(join(dir, "package.json"), JSON.stringify({ name: "fixture" }));
    const target = join(dir, "config.js");
    writeFileSync(target, 'const apiKey = "sk_live_1234567890abcdef";\n');

    const first = executeReview({
      rootDir: dir,
      scope: { kind: "files", paths: [target] },
      depth: "full",
      packIds: ["debuggatha/release"],
      policyId: undefined,
      sourceName: "test",
    });
    const [firstFinding] = first.result.findings;
    if (!firstFinding) throw new Error("expected at least one finding");
    const fingerprint = real.computeFindingFingerprint(firstFinding);

    let store = real.createEmptyMemoryStore(dir);
    const item = real.createMemoryItem({
      category: "suppression",
      confidence: "inferred",
      rationale: "Looked like a fixture, unconfirmed.",
      evidence: [{ file: target, detail: "auto-detected" }],
      fingerprintFile: fingerprint.file,
      fingerprintRuleId: fingerprint.ruleId,
      fingerprintCategory: fingerprint.category,
      origin: { kind: "detection", sessionId: "s" },
    });
    store = real.addMemoryItem(store, item); // left at "detected" — never suggested/confirmed/activated
    real.saveMemoryStore(store);

    const second = executeReview({
      rootDir: dir,
      scope: { kind: "files", paths: [target] },
      depth: "full",
      packIds: ["debuggatha/release"],
      policyId: undefined,
      sourceName: "test",
    });

    expect(second.result.findings.length).toBeGreaterThan(0);
    expect(second.suppressedFindings).toEqual([]);
  });
});
