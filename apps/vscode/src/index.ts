import { existsSync } from "node:fs";
import { join } from "node:path";
import type { Finding } from "@debuggatha/engine";
import * as vscode from "vscode";
import { registerLifecycleCommands } from "./commands/lifecycle.js";
import {
  requestForActiveFile,
  requestForDiff,
  requestForSelection,
  runReview,
} from "./commands/review.js";
import { registerReviewOnSave } from "./commands/reviewOnSave.js";
import { suppressInline } from "./commands/suppress.js";
import { getSettings, onSettingsChanged } from "./config/settings.js";
import { clearDiagnostics, initializeDiagnostics } from "./diagnostics/diagnostics.js";
import { Logger } from "./logging.js";
import { FindingCodeActionProvider } from "./providers/codeActions.js";
import { FindingDocProvider } from "./providers/findingDocProvider.js";
import { FindingsProvider } from "./providers/findingsProvider.js";
import { FindingHoverProvider } from "./providers/hover.js";
import { IntelligenceProvider } from "./providers/intelligenceProvider.js";
import { ReviewCoordinator } from "./review/coordinator.js";
import { createInProcessRunner, createProcessRunner } from "./runner/process-runner.js";
import { Services } from "./services.js";
import { StatusBar } from "./status/statusBar.js";

let services: Services | undefined;

export function activate(context: vscode.ExtensionContext) {
  const output = vscode.window.createOutputChannel("Debuggatha");
  context.subscriptions.push(output);
  const logger = new Logger(output, () => getSettings().logLevel);

  // Reviews run in their own process so a slow analyzer cannot freeze the editor and a review
  // can be cancelled. Without the worker file (a bare checkout) they run in this process.
  const workerPath = join(context.extensionPath ?? "", "dist", "review-worker.js");
  const runner = existsSync(workerPath) ? createProcessRunner(workerPath) : createInProcessRunner();
  if (!existsSync(workerPath)) logger.info("No review worker found; reviews run in this process");

  const status = new StatusBar(context);
  // "Cancel Review" is offered in the palette only while a review is running.
  const coordinator = new ReviewCoordinator(runner, (running) => {
    void vscode.commands.executeCommand(
      "setContext",
      "debuggatha.reviewing",
      running !== undefined,
    );
  });

  const findingsProvider = new FindingsProvider();
  vscode.window.registerTreeDataProvider("debuggatha-findings", findingsProvider);
  const intelligenceProvider = new IntelligenceProvider();
  vscode.window.registerTreeDataProvider("debuggatha-intelligence", intelligenceProvider);
  const docProvider = new FindingDocProvider();
  context.subscriptions.push(
    vscode.workspace.registerTextDocumentContentProvider(FindingDocProvider.scheme, docProvider),
  );

  const shared = new Services(
    logger,
    coordinator,
    findingsProvider,
    intelligenceProvider,
    docProvider,
    status,
  );
  services = shared;
  initializeDiagnostics(context);

  // Restore what the previous session found, so a reload does not look like a clean repository.
  shared.refreshFromLedger();

  context.subscriptions.push(
    vscode.languages.registerHoverProvider(
      { scheme: "file" },
      new FindingHoverProvider(shared.index),
    ),
    vscode.languages.registerCodeActionsProvider(
      { scheme: "file" },
      new FindingCodeActionProvider(shared.index),
      { providedCodeActionKinds: FindingCodeActionProvider.kinds },
    ),
  );

  const openFinding = async (finding: Finding) => {
    const uri = docProvider.registerFinding(finding);
    const doc = await vscode.workspace.openTextDocument(uri);
    await vscode.window.showTextDocument(doc, { preview: true, viewColumn: vscode.ViewColumn.Two });
  };

  context.subscriptions.push(
    vscode.commands.registerCommand("debuggatha.reviewWorkspace", () =>
      runReview(shared, { kind: "workspace" }, { origin: "manual" }),
    ),
    vscode.commands.registerCommand("debuggatha.reviewDiff", async () => {
      const rootDir = shared.rootDir;
      if (!rootDir) {
        vscode.window.showErrorMessage("No workspace folder open.");
        return;
      }
      await runReview(shared, await requestForDiff(rootDir), { origin: "manual" });
    }),
    vscode.commands.registerCommand("debuggatha.reviewActiveFile", async () => {
      const request = requestForActiveFile();
      if (!request) {
        vscode.window.showErrorMessage("No active file to review.");
        return;
      }
      await runReview(shared, request, { origin: "manual" });
    }),
    vscode.commands.registerCommand("debuggatha.reviewSelection", async () => {
      const rootDir = shared.rootDir;
      const request = rootDir ? requestForSelection(rootDir) : undefined;
      if (!request) {
        vscode.window.showInformationMessage("Select some code to review first.");
        return;
      }
      await runReview(shared, request, { origin: "manual" });
    }),
    vscode.commands.registerCommand("debuggatha.reviewActiveFileWithModel", async () => {
      const request = requestForActiveFile();
      if (!request) {
        vscode.window.showErrorMessage("No active file to review.");
        return;
      }
      await runReview(shared, request, { origin: "manual", semantic: true });
    }),
    vscode.commands.registerCommand("debuggatha.cancelReview", () => coordinator.cancel()),
    vscode.commands.registerCommand("debuggatha.showLog", () => output.show(true)),
    vscode.commands.registerCommand("debuggatha.openFinding", openFinding),
    vscode.commands.registerCommand("debuggatha.openFindingById", async (id: string) => {
      const entry = shared.index.entry(id);
      if (entry) await openFinding(entry.latestFinding);
    }),
    vscode.commands.registerCommand("debuggatha.suppressInline", (id: string, reason?: string) =>
      suppressInline(shared, id, reason),
    ),
  );

  registerLifecycleCommands(context, shared, findingsProvider);
  registerReviewOnSave(context, shared);

  context.subscriptions.push(
    onSettingsChanged(() => {
      // The minimum severity decides which findings the editor shows.
      shared.refreshFromLedger();
    }),
  );
}

export function deactivate() {
  // A review still running is stopped, along with any analyzer it started.
  services?.coordinator.cancel();
  services = undefined;
  clearDiagnostics();
}
