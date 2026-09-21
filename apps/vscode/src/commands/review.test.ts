import { chmodSync, existsSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { delimiter, join } from "node:path";
import { loadLedger } from "@debuggatha/engine";
import { afterAll, afterEach, beforeEach, describe, expect, it } from "vitest";
import { git, makeRepo, removeRepos } from "../test/fixtures.js";
import { editorDiagnostics, harness, statusItem } from "../test/harness.js";
import { documentFor, mock, resetMock, Uri } from "../test/vscode-mock.js";
import { requestForActiveFile, requestForDiff, requestForSelection, runReview } from "./review.js";

afterAll(removeRepos);

const infos = () => mock.infos.join("\n");
const rulesInLedger = (root: string) =>
  loadLedger(root).entries.flatMap((entry) =>
    entry.latestFinding.evidence.flatMap((e) => ("ruleId" in e ? [e.ruleId] : [])),
  );

describe("reviewing", () => {
  beforeEach(resetMock);

  it("reviews the workspace end to end: ledger, tree, editor, intelligence, message, and status", async () => {
    const { root, badFile } = makeRepo();
    const { services } = harness(root);

    await runReview(services, { kind: "workspace" }, { origin: "manual" });

    expect(mock.errors).toEqual([]);
    expect(infos()).toMatch(/Review complete: \d+ findings/);
    expect(existsSync(join(root, ".debuggatha", "ledger.json"))).toBe(true);
    expect(mock.collections.get("debuggatha")?.entries.has(badFile)).toBe(true);
    expect(editorDiagnostics().flat().length).toBe(loadLedger(root).entries.length);
    expect((await services.findings.getChildren()).length).toBeGreaterThan(0);
    expect((await services.intelligence.getChildren()).length).toBe(4);
    expect(statusItem()?.text).toContain(String(loadLedger(root).entries.length));
    expect(statusItem()?.command).not.toBe("debuggatha.cancelReview");
  });

  it("applies packs detected from the repository without any setting", async () => {
    const { root } = makeRepo();
    const { services } = harness(root);
    await runReview(services, { kind: "workspace" }, { origin: "manual" });
    expect(rulesInLedger(root)).toContain("no-eval");
    expect(rulesInLedger(root)).toContain("no-explicit-any");
  });

  it("reviews only the active file", async () => {
    const { root, badFile } = makeRepo();
    const { services } = harness(root);
    mock.activeTextEditor = { document: { uri: Uri.file(badFile) } };
    const request = requestForActiveFile();
    expect(request).toEqual({ kind: "file", path: badFile });

    await runReview(services, request as never, { origin: "manual" });

    const files = loadLedger(root).entries.map((entry) => entry.latestFinding.locations[0]?.file);
    expect(new Set(files)).toEqual(new Set(["src/bad.ts"]));
  });

  it("has no request for a file when none is open", () => {
    harness();
    expect(requestForActiveFile()).toBeUndefined();
  });

  it("does not track the same finding twice after a file review and a workspace review", async () => {
    const { root, badFile } = makeRepo();
    const { services } = harness(root);
    await runReview(services, { kind: "file", path: badFile }, { origin: "manual" });
    const afterFile = loadLedger(root).entries.length;
    await runReview(services, { kind: "workspace" }, { origin: "manual" });
    expect(loadLedger(root).entries.length).toBe(afterFile);
  });

  it("reviews only what changed in the working tree for a diff review", async () => {
    const { root, badFile } = makeRepo("var old: any = 1;\n");
    git(root, "init", "-q", "-b", "main");
    git(root, "add", "-A");
    git(root, "commit", "-q", "-m", "base");
    writeFileSync(badFile, 'var old: any = 1;\neval("new");\n');
    const { services } = harness(root);

    await runReview(services, await requestForDiff(root), { origin: "manual" });

    expect(rulesInLedger(root)).toContain("no-eval");
    expect(rulesInLedger(root)).not.toContain("no-explicit-any");
  });

  it("reviews only the selected lines, at their real line numbers", async () => {
    const { root, badFile } = makeRepo();
    const { services } = harness(root);
    // Lines 2 and 3 of the fixture: the `eval` and the hardcoded password. Line 1 (`any`) is left out.
    mock.activeTextEditor = {
      document: documentFor(badFile),
      selection: {
        isEmpty: false,
        start: { line: 1, character: 0 },
        end: { line: 2, character: 999 },
      },
    } as never;
    const request = requestForSelection(root);
    expect(request).toMatchObject({ kind: "selection", path: badFile });

    await runReview(services, request as never, { origin: "manual" });

    const rules = rulesInLedger(root);
    expect(rules).toContain("no-eval");
    expect(rules).not.toContain("no-explicit-any");
    const lines = loadLedger(root).entries.map((e) => e.latestFinding.locations[0]?.lines?.start);
    expect(lines.every((line) => line === 2 || line === 3)).toBe(true);
  });

  it("a selection that ends at the start of a line does not include that line", () => {
    const { root, badFile } = makeRepo();
    harness(root);
    mock.activeTextEditor = {
      document: documentFor(badFile),
      selection: {
        isEmpty: false,
        start: { line: 0, character: 0 },
        end: { line: 1, character: 0 },
      },
    } as never;
    const request = requestForSelection(root);
    expect((request as { diff: string }).diff).toContain("@@ -0,0 +1,1 @@");
  });

  it("has no request for an empty selection", () => {
    const { root, badFile } = makeRepo();
    harness(root);
    mock.activeTextEditor = {
      document: documentFor(badFile),
      selection: {
        isEmpty: true,
        start: { line: 0, character: 0 },
        end: { line: 0, character: 0 },
      },
    } as never;
    expect(requestForSelection(root)).toBeUndefined();
  });

  it("reports a clean review as zero findings", async () => {
    const { root } = makeRepo("export const ok = 1;\n");
    const { services } = harness(root);
    await runReview(services, { kind: "workspace" }, { origin: "manual" });
    expect(infos()).toContain("Review complete: 0 findings");
    expect(editorDiagnostics().flat()).toHaveLength(0);
    expect(statusItem()?.text).toBe("$(check-all) Debuggatha");
  });

  it("explains that a workspace is needed", async () => {
    const { services } = harness();
    await runReview(services, { kind: "workspace" }, { origin: "manual" });
    expect(mock.errors).toEqual(["No workspace folder open."]);
  });

  it("adds the packs a user configured on top of the detected ones", async () => {
    const { root } = makeRepo();
    const { services } = harness(root);
    mock.settings["debuggatha.extraReviewPacks"] = ["debuggatha/owasp"];
    await runReview(services, { kind: "workspace" }, { origin: "manual" });
    expect(mock.errors).toEqual([]);
    expect(infos()).toMatch(/Review complete: [1-9]\d* findings/);
  });

  it("a person's review has a cancellable notification; a save's is only in the status bar", async () => {
    const { root, badFile } = makeRepo();
    const { services } = harness(root);
    await runReview(services, { kind: "workspace" }, { origin: "manual" });
    await runReview(services, { kind: "file", path: badFile }, { origin: "save", widen: false });
    expect(mock.progressLocations).toEqual([15, 10]); // Notification, Window
  });

  it("a save says nothing: no message, and a failure does not interrupt the person typing", async () => {
    const { root, badFile } = makeRepo();
    const { services } = harness(root);
    mock.settings["debuggatha.extraReviewPacks"] = ["debuggatha/does-not-exist"];
    await runReview(services, { kind: "file", path: badFile }, { origin: "save", widen: false });
    expect(mock.infos).toEqual([]);
    expect(mock.errors).toEqual([]);
    expect(statusItem()?.text).toContain("$(error)");
  });

  it("surfaces a failed review with the log and a way to retry, and does not stay in the reviewing state", async () => {
    const { root } = makeRepo();
    const { services } = harness(root);
    mock.settings["debuggatha.extraReviewPacks"] = ["debuggatha/does-not-exist"];
    mock.errorChoice = "Show Log";

    await runReview(services, { kind: "workspace" }, { origin: "manual" });

    expect(mock.errors[0]).toContain("Debuggatha review failed");
    expect(mock.executed).toContainEqual({ command: "debuggatha.showLog", args: [] });
    expect(statusItem()?.text).toContain("$(error)");
    expect(mock.outputLines.some((line) => line.includes("Review failed"))).toBe(true);
  });

  it("only shows findings at or above the minimum severity in the editor, and the tree keeps all of them", async () => {
    const { root } = makeRepo();
    const { services } = harness(root);
    await runReview(services, { kind: "workspace" }, { origin: "manual" });
    const total = loadLedger(root).entries.length;

    mock.settings["debuggatha.minimumSeverity"] = "high";
    services.refreshFromLedger();

    const shown = editorDiagnostics().flat().length;
    expect(shown).toBeGreaterThan(0);
    expect(shown).toBeLessThan(total);
    expect((await services.findings.getChildren()).length).toBeGreaterThan(0);
  });
});

describe("what depth and trust change", () => {
  let bin: string;
  let savedPath: string | undefined;
  beforeEach(() => {
    resetMock();
    savedPath = process.env.PATH;
    bin = mkdtempSync(join(tmpdir(), "debuggatha-vsc-bin-"));
    // A `ruff` that records that it ran and reports one finding in app.py.
    writeFileSync(
      join(bin, "ruff"),
      `#!/bin/sh
if [ "$1" = "--version" ]; then echo "ruff 0.15.1"; exit 0; fi
touch "${join(bin, "ruff-ran")}"
echo '[{"code":"F401","message":"unused import","filename":"'"$PWD"'/app.py","location":{"row":1,"column":8}}]'
`,
    );
    chmodSync(join(bin, "ruff"), 0o755);
    process.env.PATH = [bin, "/usr/bin", "/bin"].join(delimiter);
  });
  afterEach(() => {
    process.env.PATH = savedPath;
    rmSync(bin, { recursive: true, force: true });
  });

  const withPython = () => {
    const { root } = makeRepo();
    writeFileSync(join(root, "app.py"), "import os\n");
    return root;
  };
  const ranRuff = () => existsSync(join(bin, "ruff-ran"));

  it("full also runs the external analyzers installed on the machine", async () => {
    const root = withPython();
    const { services } = harness(root);
    mock.settings["debuggatha.reviewDepth"] = "full";
    await runReview(services, { kind: "workspace" }, { origin: "manual" });
    expect(ranRuff()).toBe(true);
    expect(rulesInLedger(root)).toContain("no-eval"); // the built-in detectors still run
    expect(loadLedger(root).entries.some((e) => e.fingerprint.ruleId === "ruff:F401")).toBe(true);
  });

  it("quick runs the built-in detectors only", async () => {
    const root = withPython();
    const { services } = harness(root);
    mock.settings["debuggatha.reviewDepth"] = "quick";
    await runReview(services, { kind: "workspace" }, { origin: "manual" });
    expect(ranRuff()).toBe(false);
    expect(rulesInLedger(root)).toContain("no-eval");
  });

  it("an untrusted workspace never runs an analyzer, and says why", async () => {
    const root = withPython();
    const { services } = harness(root);
    mock.trusted = false;
    await runReview(services, { kind: "workspace" }, { origin: "manual" });
    expect(ranRuff()).toBe(false);
    expect(infos()).toContain("not trusted");
    expect(rulesInLedger(root)).toContain("no-eval");
  });

  it("architectural widens a file review to the whole repository, and a save never does", async () => {
    const root = withPython();
    const { services } = harness(root);
    const bad = join(root, "src", "bad.ts");
    mock.settings["debuggatha.reviewDepth"] = "architectural";

    await runReview(services, { kind: "file", path: join(root, "app.py") }, { origin: "manual" });
    const files = new Set(loadLedger(root).entries.map((e) => e.fingerprint.file));
    expect(files).toContain("src/bad.ts"); // reviewed although only app.py was asked
    expect(infos()).toContain("whole repository");

    rmSync(join(root, ".debuggatha"), { recursive: true, force: true });
    mock.infos = [];
    await runReview(services, { kind: "file", path: bad }, { origin: "save", widen: false });
    expect(new Set(loadLedger(root).entries.map((e) => e.fingerprint.file))).toEqual(
      new Set(["src/bad.ts"]),
    );
  });

  it("depth changes are picked up on the next review without reloading", async () => {
    const root = withPython();
    const { services } = harness(root);
    mock.settings["debuggatha.reviewDepth"] = "quick";
    await runReview(services, { kind: "workspace" }, { origin: "manual" });
    expect(ranRuff()).toBe(false);
    mock.settings["debuggatha.reviewDepth"] = "full";
    await runReview(services, { kind: "workspace" }, { origin: "manual" });
    expect(ranRuff()).toBe(true);
  });
});

describe("the model review", () => {
  beforeEach(resetMock);

  it("asks the person to choose a model first, and sends nothing anywhere until they do", async () => {
    const { root, badFile } = makeRepo();
    const { services } = harness(root);
    await runReview(
      services,
      { kind: "file", path: badFile },
      { origin: "manual", semantic: true },
    );
    expect(infos()).toContain("Choose a model on this machine");
    expect(existsSync(join(root, ".debuggatha"))).toBe(false);
  });

  it("logs what a review did, and never the code it read", async () => {
    const { root } = makeRepo();
    const { services } = harness(root);
    await runReview(services, { kind: "workspace" }, { origin: "manual" });
    const log = mock.outputLines.join("\n");
    expect(log).toContain("Review started");
    expect(log).toContain("Review finished");
    expect(log).not.toContain("hunter22222");
    expect(log).not.toContain("eval(");
  });
});
