import * as assert from "node:assert";
import { existsSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import * as vscode from "vscode";

const EXTENSION_ID = "SxnnysideProject.debuggatha";

const workspace = process.env.DEBUGGATHA_TEST_WORKSPACE as string;
const pidFile = process.env.DEBUGGATHA_TEST_RUFF_PID as string;
const badFile = join(workspace, "src", "bad.ts");
const ledgerDir = join(workspace, ".debuggatha");
const ledgerPath = join(ledgerDir, "ledger.json");
const slowMarker = join(workspace, ".slow-ruff");

/** Lines 1, 2, 3: an `any` and a `var`, `==` and `eval`, a hardcoded password. */
const ORIGINAL = 'var x: any = 1;\nif (x == 2) { eval("1"); }\nconst password = "hunter22222";\n';

interface LedgerEntry {
  id: string;
  status: string;
  fingerprint: { file: string; ruleId?: string };
  latestFinding: { locations: { lines?: { start: number } }[] };
}
const ledger = (): LedgerEntry[] =>
  existsSync(ledgerPath) ? JSON.parse(readFileSync(ledgerPath, "utf8")).entries : [];
const idOf = (rule: string) => {
  const entry = ledger().find((e) => e.fingerprint.ruleId === rule);
  assert.ok(entry, `no ${rule} finding in the ledger`);
  return entry.id;
};
const badUri = vscode.Uri.file(badFile);
const diagnostics = () =>
  vscode.languages.getDiagnostics(badUri).filter((d) => d.source === "Debuggatha");
const config = () => vscode.workspace.getConfiguration("debuggatha");
const setting = (key: string, value: unknown) =>
  config().update(key, value, vscode.ConfigurationTarget.Workspace);

async function until<T>(what: string, probe: () => T | undefined | false, ms = 30_000): Promise<T> {
  const deadline = Date.now() + ms;
  while (Date.now() < deadline) {
    const value = probe();
    if (value) return value;
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
  throw new Error(`Timed out waiting for ${what}`);
}

const alive = (pid: number) => {
  try {
    process.kill(pid, 0);
    return true;
  } catch {
    return false;
  }
};

/**
 * Opens the fixture as `reset` left it. VS Code keeps a document's model after its editor closes,
 * so a file rewritten on disk is only picked up when its watcher fires: wait for it, and drop
 * unsaved edits from an earlier test.
 */
async function openBad() {
  const document = await vscode.workspace.openTextDocument(badUri);
  const editor = await vscode.window.showTextDocument(document);
  if (document.isDirty) await vscode.commands.executeCommand("workbench.action.files.revert");
  await until("the fixture to be as reset left it", () => document.getText() === ORIGINAL);
  return { document, editor };
}

async function review() {
  await vscode.commands.executeCommand("debuggatha.reviewWorkspace");
  await until("diagnostics", () => diagnostics().length > 0);
}

/** A blank slate: the file as it was, no ledger, default settings, nothing slow. */
async function reset() {
  await vscode.commands.executeCommand("workbench.action.closeAllEditors");
  writeFileSync(badFile, ORIGINAL);
  rmSync(ledgerDir, { recursive: true, force: true });
  rmSync(slowMarker, { force: true });
  rmSync(pidFile, { force: true });
  for (const key of ["reviewDepth", "reviewOnSave", "minimumSeverity", "extraReviewPacks"]) {
    await setting(key, undefined);
  }
  // Findings from the last run are still in the editor; a review with no ledger clears them.
  rmSync(ledgerDir, { recursive: true, force: true });
  await vscode.commands.executeCommand("debuggatha.reviewWorkspace");
  rmSync(ledgerDir, { recursive: true, force: true });
  rmSync(pidFile, { force: true });
}

suite("Debuggatha in a real VS Code", () => {
  suiteSetup(async () => {
    const extension = vscode.extensions.getExtension(EXTENSION_ID);
    assert.ok(extension, `extension ${EXTENSION_ID} was not loaded`);
    await extension.activate();
  });

  setup(reset);

  test("declares every command it contributes, and registers them all", async () => {
    const extension = vscode.extensions.getExtension(EXTENSION_ID);
    const declared: string[] = extension?.packageJSON.contributes.commands.map(
      (c: { command: string }) => c.command,
    );
    assert.ok(declared.length >= 15);
    const registered = await vscode.commands.getCommands(true);
    for (const command of declared) {
      assert.ok(registered.includes(command), `${command} is not registered`);
    }
  });

  test("contributes exactly the settings something reads, with the values the code expects", () => {
    const cfg = config();
    assert.deepStrictEqual(cfg.get("extraReviewPacks"), []);
    assert.strictEqual(cfg.get("reviewDepth"), "full");
    assert.strictEqual(cfg.get("logLevel"), "info");
    assert.strictEqual(cfg.get("reviewOnSave"), false);
    assert.strictEqual(cfg.get("minimumSeverity"), "informational");
    assert.deepStrictEqual(cfg.get("enabledAnalyzers"), []);
    assert.strictEqual(cfg.get("semantic.provider"), "off");
    for (const removed of ["runtime", "transport"]) {
      assert.strictEqual(cfg.inspect(removed)?.defaultValue, undefined, `${removed} came back`);
    }
  });

  test("a workspace cannot switch on what runs code or sends it to a model", () => {
    const extension = vscode.extensions.getExtension(EXTENSION_ID);
    const properties = extension?.packageJSON.contributes.configuration.properties;
    for (const key of [
      "debuggatha.enabledAnalyzers",
      "debuggatha.semantic.provider",
      "debuggatha.semantic.model",
      "debuggatha.semantic.url",
    ]) {
      assert.strictEqual(properties[key].scope, "machine", `${key} must be machine scope`);
    }
    assert.strictEqual(
      extension?.packageJSON.capabilities.untrustedWorkspaces.supported,
      "limited",
    );
  });

  test("reviews the workspace and shows findings in the editor's problem list", async () => {
    await review();
    assert.ok(existsSync(ledgerPath), "the ledger was not written");
    assert.strictEqual(diagnostics().length, ledger().length);
    assert.ok(diagnostics().some((d) => d.severity === vscode.DiagnosticSeverity.Error));
  });

  test("underlines exactly what was found, and names the rule", async () => {
    await review();
    const { document } = await openBad();
    const any = diagnostics().find((d) => d.code === "no-explicit-any");
    assert.ok(any, "no `any` diagnostic");
    assert.strictEqual(document.getText(any.range), ": any");
    assert.strictEqual(any.source, "Debuggatha");
    const evalRule = diagnostics().find((d) => d.code === "no-eval");
    assert.ok(evalRule && document.getText(evalRule.range).startsWith("eval("));
  });

  test("hovering a finding shows the rule, why, and the fix", async () => {
    await review();
    await openBad();
    const hovers = (await vscode.commands.executeCommand(
      "vscode.executeHoverProvider",
      badUri,
      new vscode.Position(0, 1), // on `var`
    )) as vscode.Hover[];
    const text = hovers
      .flatMap((hover) => hover.contents)
      .map((content) => (typeof content === "string" ? content : content.value))
      .join("\n");
    assert.ok(text.includes("`no-var`"), text);
    assert.ok(text.includes("**Fix:**"), text);
    assert.ok(text.includes("// before") && text.includes("// after"), text);
    assert.ok(text.includes("debuggatha.resolveFindingById"), "no way to act from the hover");
  });

  test("offers the exact fix as a quick fix, applies it, and stops offering it once the line changed", async () => {
    await review();
    const { document } = await openBad();
    const line = new vscode.Range(0, 0, 0, 3);
    // VS Code cancels a code-action request when the document changes under it (its file watcher
    // may still be applying what `reset` wrote), so ask again.
    const actionsAt = async (): Promise<vscode.CodeAction[]> => {
      for (let attempt = 0; ; attempt++) {
        try {
          return (await vscode.commands.executeCommand(
            "vscode.executeCodeActionProvider",
            badUri,
            line,
            vscode.CodeActionKind.QuickFix.value,
          )) as vscode.CodeAction[];
        } catch (error) {
          if (attempt >= 5 || !String(error).includes("Canceled")) throw error;
          await new Promise((resolve) => setTimeout(resolve, 300));
        }
      }
    };

    const fix = (await actionsAt()).find((a) => a.edit && a.isPreferred);
    assert.ok(fix, "no preferred quick fix on `var`");
    assert.ok(fix.title.startsWith("Debuggatha:"));
    assert.ok(await vscode.workspace.applyEdit(fix.edit as vscode.WorkspaceEdit));
    assert.strictEqual(document.lineAt(0).text, "let x: any = 1;");

    // The finding is a snapshot: the line has changed, so no edit is offered for it any more.
    assert.strictEqual(
      (await actionsAt()).filter((a) => a.edit).length,
      0,
      "a stale finding still offered an edit",
    );
  });

  test("the lightbulb also offers to accept, resolve, and dismiss a finding", async () => {
    await review();
    await openBad();
    const titles = (
      (await vscode.commands.executeCommand(
        "vscode.executeCodeActionProvider",
        badUri,
        new vscode.Range(1, 0, 1, 40),
        vscode.CodeActionKind.QuickFix.value,
      )) as vscode.CodeAction[]
    ).map((a) => a.title);
    assert.ok(
      titles.some((t) => t.includes("accept no-eval here")),
      titles.join("\n"),
    );
    assert.ok(titles.some((t) => t === "Debuggatha: mark resolved"));
    assert.ok(titles.some((t) => t === "Debuggatha: dismiss"));
  });

  test("accepting a finding writes the reason beside the code, and the next review agrees", async () => {
    await review();
    const { document } = await openBad();
    const id = idOf("no-eval");

    const applied = await vscode.commands.executeCommand(
      "debuggatha.suppressInline",
      id,
      "legacy, tracked in TICKET-1",
    );
    assert.strictEqual(applied, true);
    assert.strictEqual(
      document.lineAt(1).text,
      "// debuggatha-ignore-next-line no-eval -- legacy, tracked in TICKET-1",
    );
    await document.save();

    await vscode.commands.executeCommand("debuggatha.reviewWorkspace");
    await until(
      "the eval finding to close",
      () => ledger().find((e) => e.id === id)?.status === "resolved",
    );
    assert.ok(!diagnostics().some((d) => d.code === "no-eval"));
  });

  test("walks a finding through resolve and reopen, in the ledger and in the editor", async () => {
    await review();
    const before = diagnostics().length;
    const id = ledger()[0]?.id as string;

    await vscode.commands.executeCommand("debuggatha.resolveFinding", { entry: { id } });
    assert.strictEqual(ledger().find((e) => e.id === id)?.status, "resolved");
    await until("one fewer diagnostic", () => diagnostics().length === before - 1);

    await vscode.commands.executeCommand("debuggatha.reopenFinding", { entry: { id } });
    assert.strictEqual(ledger().find((e) => e.id === id)?.status, "reopened");
    await until("the diagnostic to return", () => diagnostics().length === before);
  });

  test("opens a finding as a readable document", async () => {
    await review();
    const entries = JSON.parse(readFileSync(ledgerPath, "utf8")).entries;
    await vscode.commands.executeCommand("debuggatha.openFinding", entries[0].latestFinding);
    const document = await until("the finding document", () => {
      const active = vscode.window.activeTextEditor?.document;
      return active?.uri.scheme === "debuggatha-finding" && active;
    });
    assert.ok(document.getText().includes(entries[0].latestFinding.title));
    assert.ok(document.getText().includes("## Evidence"));
  });

  test("reviews the file that is open without tracking its findings twice", async () => {
    await review();
    const before = ledger().length;
    await openBad();
    await vscode.commands.executeCommand("debuggatha.reviewActiveFile");
    assert.strictEqual(ledger().length, before);
  });

  test("reviews only the selected lines", async () => {
    const { editor } = await openBad();
    editor.selection = new vscode.Selection(1, 0, 1, 30); // only line 2: the eval
    await vscode.commands.executeCommand("debuggatha.reviewSelection");
    await until("findings for the selection", () => ledger().length > 0);

    const rules = ledger().map((e) => e.fingerprint.ruleId);
    assert.ok(rules.includes("no-eval"));
    assert.ok(!rules.includes("no-explicit-any"), "reviewed a line that was not selected");
    assert.ok(ledger().every((e) => e.latestFinding.locations[0]?.lines?.start === 2));
  });

  test("reviews on save when asked, quietly, and only that file", async () => {
    await setting("reviewOnSave", true);
    const { document } = await openBad();
    const edit = new vscode.WorkspaceEdit();
    edit.insert(badUri, new vscode.Position(3, 0), "var later = 4;\n");
    await vscode.workspace.applyEdit(edit);
    await document.save();

    await until("a finding from the save", () =>
      diagnostics().some((d) => d.range.start.line === 3),
    );
    assert.strictEqual(
      new Set(ledger().map((e) => e.fingerprint.file)).size,
      1,
      "a save reviewed more than the saved file",
    );
  });

  test("does not review on save unless asked", async () => {
    const { document } = await openBad();
    const edit = new vscode.WorkspaceEdit();
    edit.insert(badUri, new vscode.Position(3, 0), "var later = 4;\n");
    await vscode.workspace.applyEdit(edit);
    await document.save();
    await new Promise((resolve) => setTimeout(resolve, 2_000));
    assert.strictEqual(existsSync(ledgerPath), false);
  });

  test("shows in the editor only findings at or above the minimum severity", async () => {
    await review();
    const all = diagnostics().length;
    await setting("minimumSeverity", "high");
    await until("fewer diagnostics", () => diagnostics().length < all && diagnostics().length > 0);
    assert.ok(
      diagnostics().every((d) => d.severity === vscode.DiagnosticSeverity.Error),
      "a lower severity is still shown",
    );
    assert.strictEqual(ledger().length, all, "the ledger must keep everything");
  });

  test("quick runs the built-in detectors only; full also runs the installed analyzers", async () => {
    await setting("reviewDepth", "quick");
    await vscode.commands.executeCommand("debuggatha.reviewWorkspace");
    assert.strictEqual(existsSync(pidFile), false, "quick ran an external analyzer");

    await setting("reviewDepth", "full");
    await vscode.commands.executeCommand("debuggatha.reviewWorkspace");
    assert.strictEqual(existsSync(pidFile), true, "full did not run the analyzer");
  });

  test("cancelling stops the review and the analyzer it started, and leaves the ledger readable", async () => {
    await review(); // a first review: the fake analyzer answers at once, without the marker
    rmSync(pidFile, { force: true });
    const before = readFileSync(ledgerPath, "utf8");

    writeFileSync(slowMarker, "slow");
    const started = Date.now();
    const running = Promise.resolve(vscode.commands.executeCommand("debuggatha.reviewWorkspace"));
    const pid = Number(
      await until(
        "the analyzer to start",
        () => existsSync(pidFile) && readFileSync(pidFile, "utf8").trim(),
      ),
    );
    assert.ok(alive(pid), "the analyzer is not running");

    await vscode.commands.executeCommand("debuggatha.cancelReview");
    await running;

    assert.ok(Date.now() - started < 20_000, "cancelling did not stop the review promptly");
    await until("the analyzer to be stopped", () => !alive(pid), 10_000);
    assert.doesNotThrow(() => JSON.parse(readFileSync(ledgerPath, "utf8")), "the ledger is broken");
    assert.strictEqual(
      readFileSync(ledgerPath, "utf8"),
      before,
      "a cancelled review changed the ledger",
    );
  });

  test("the log can be opened", async () => {
    await vscode.commands.executeCommand("debuggatha.showLog");
  });
});
