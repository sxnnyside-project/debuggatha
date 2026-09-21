import { createFinding, type Finding, type LedgerEntry, loadLedger } from "@debuggatha/engine";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { FindingIndex } from "../findings/index.js";
import { makeRepo, removeRepos, seedLedger } from "../test/fixtures.js";
import { DiagnosticSeverity, mock, resetMock } from "../test/vscode-mock.js";
import { clearDiagnostics, initializeDiagnostics, updateDiagnostics } from "./diagnostics.js";

afterEach(removeRepos);

function entryFor(
  finding: Finding,
  status: LedgerEntry["status"] = "open",
  id = finding.id,
): LedgerEntry {
  return {
    id,
    fingerprint: { file: finding.locations[0]?.file ?? "" },
    status,
    latestFinding: finding,
  } as never;
}

function finding(
  over: { file?: string; line?: number; severity?: Finding["severity"]; title?: string } = {},
) {
  const {
    file = "src/a.ts",
    line = 3,
    severity = "high",
    title = "Avoid using the `any` type",
  } = over;
  return createFinding({
    title,
    explanation: "why",
    severity,
    confidence: "medium",
    category: "maintainability",
    locations: [{ file, lines: { start: line, end: line }, columns: { start: 8, end: 13 } }],
    evidence: [{ kind: "criteria", ruleId: "no-explicit-any", source: file }],
  });
}

const collection = () => {
  const found = mock.collections.get("debuggatha");
  if (!found) throw new Error("collection not created");
  return found;
};

function show(entries: LedgerEntry[], minimum: Finding["severity"] = "informational") {
  const index = new FindingIndex();
  index.rebuild(entries, "/repo", minimum);
  updateDiagnostics(index);
}

describe("diagnostics", () => {
  beforeEach(() => {
    resetMock();
    initializeDiagnostics({ subscriptions: [] } as never);
  });

  it("creates one collection named after the extension and owns its disposal", () => {
    resetMock();
    const subscriptions: unknown[] = [];
    initializeDiagnostics({ subscriptions } as never);
    expect(mock.collections.has("debuggatha")).toBe(true);
    expect(subscriptions).toHaveLength(1);
  });

  it("underlines exactly what the detector matched, and carries the rule as its code", () => {
    show([entryFor(finding({ line: 3 }))]);
    const [diagnostic] = collection().entries.get("/repo/src/a.ts") ?? [];
    expect(diagnostic?.range.startLine).toBe(2);
    expect(diagnostic?.range.startCharacter).toBe(7);
    expect(diagnostic?.range.endCharacter).toBe(12);
    expect(diagnostic?.message).toBe("Avoid using the `any` type");
    expect(diagnostic?.source).toBe("Debuggatha");
    expect(diagnostic?.code).toBe("no-explicit-any");
  });

  it("links an external tool's rule to its documentation", () => {
    const external = createFinding({
      title: "unused import",
      explanation: "x",
      severity: "medium",
      confidence: "high",
      category: "reliability",
      locations: [{ file: "a.py", lines: { start: 1, end: 1 } }],
      evidence: [
        {
          kind: "external-analyzer",
          tool: "ruff",
          ruleId: "F401",
          version: undefined,
          license: "MIT",
          url: "https://docs.astral.sh/ruff/rules/unused-import",
        },
      ],
    });
    show([entryFor(external)]);
    const [diagnostic] = collection().entries.get("/repo/a.py") ?? [];
    const code = diagnostic?.code as { value: string; target: { toString(): string } } | undefined;
    expect(code?.value).toBe("ruff:F401");
    expect(String(code?.target)).toContain("docs.astral.sh");
  });

  it("maps severities onto editor severities", () => {
    const severities = ["critical", "high", "medium", "low", "informational"] as const;
    show(severities.map((severity, i) => entryFor(finding({ severity }), "open", `e${i}`)));
    const mapped = (collection().entries.get("/repo/src/a.ts") ?? []).map((d) => d.severity);
    expect(mapped).toEqual([
      DiagnosticSeverity.Error,
      DiagnosticSeverity.Error,
      DiagnosticSeverity.Warning,
      DiagnosticSeverity.Information,
      DiagnosticSeverity.Hint,
    ]);
  });

  it("shows only findings at or above the minimum severity", () => {
    const entries = (["critical", "medium", "low"] as const).map((severity, i) =>
      entryFor(finding({ severity }), "open", `e${i}`),
    );
    show(entries, "medium");
    expect(collection().entries.get("/repo/src/a.ts")).toHaveLength(2);
    show(entries, "critical");
    expect(collection().entries.get("/repo/src/a.ts")).toHaveLength(1);
  });

  it("shows only findings that are still open", () => {
    show([
      entryFor(finding(), "open", "a"),
      entryFor(finding(), "resolved", "b"),
      entryFor(finding(), "dismissed", "c"),
      entryFor(finding(), "reopened", "d"),
    ]);
    expect(collection().entries.get("/repo/src/a.ts")).toHaveLength(2);
  });

  it("groups findings by file, resolving the paths the ledger stores against the workspace", () => {
    show([
      entryFor(finding({ file: "a.ts" }), "open", "a"),
      entryFor(finding({ file: "b.ts" }), "open", "b"),
      entryFor(finding({ file: "a.ts" }), "open", "c"),
    ]);
    expect(collection().entries.get("/repo/a.ts")).toHaveLength(2);
    expect(collection().entries.get("/repo/b.ts")).toHaveLength(1);
  });

  it("replaces earlier diagnostics instead of accumulating them", () => {
    show([entryFor(finding({ file: "old.ts" }))]);
    show([entryFor(finding({ file: "new.ts" }))]);
    expect([...collection().entries.keys()]).toEqual(["/repo/new.ts"]);
  });

  it("works on what a real review produced", () => {
    const { root } = makeRepo();
    seedLedger(root);
    const entries = [...loadLedger(root).entries];
    const index = new FindingIndex();
    index.rebuild(entries, root, "informational");
    updateDiagnostics(index);
    expect((collection().entries.get(`${root}/src/bad.ts`) ?? []).length).toBe(entries.length);
  });

  it("clears everything", () => {
    show([entryFor(finding())]);
    clearDiagnostics();
    expect(collection().entries.size).toBe(0);
  });
});
