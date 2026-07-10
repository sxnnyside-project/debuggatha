import type { Finding } from "@debuggatha/core";
import { ledgerFilePath, loadLedger } from "@debuggatha/core";
import * as vscode from "vscode";
import { registerLifecycleCommands } from "./commands/lifecycle.js";
import { executeReview } from "./commands/review.js";
import { onSettingsChanged } from "./config/settings.js";
import { clearDiagnostics, initializeDiagnostics } from "./diagnostics/diagnostics.js";
import { FindingDocProvider } from "./providers/findingDocProvider.js";
import { FindingsProvider } from "./providers/findingsProvider.js";
import { IntelligenceProvider } from "./providers/intelligenceProvider.js";
import { initializeStatusBar, updateStatusBarStatus } from "./status/statusBar.js";

export function activate(context: vscode.ExtensionContext) {
  // 1. Initialize Providers
  const findingsProvider = new FindingsProvider();
  vscode.window.registerTreeDataProvider("debuggatha-findings", findingsProvider);

  const intelligenceProvider = new IntelligenceProvider();
  vscode.window.registerTreeDataProvider("debuggatha-intelligence", intelligenceProvider);

  const docProvider = new FindingDocProvider();
  context.subscriptions.push(
    vscode.workspace.registerTextDocumentContentProvider(FindingDocProvider.scheme, docProvider),
  );

  // 2. Initialize Status Bar & Diagnostics
  initializeStatusBar(context);
  initializeDiagnostics(context);

  // Load existing findings if ledger exists
  if (vscode.workspace.workspaceFolders && vscode.workspace.workspaceFolders.length > 0) {
    const folders = vscode.workspace.workspaceFolders || [];
    const rootDir = folders[0]?.uri?.fsPath;
    if (!rootDir) return;
    try {
      const ledger = loadLedger(ledgerFilePath(rootDir));
      const ledgerFindings = ledger.entries.map((e: any) => e.latestFinding);
      findingsProvider.refresh(ledgerFindings);
      // Wait to populate diagnostics maybe? Or do it directly
      // updateDiagnostics(ledgerFindings);
    } catch (e) {
      // Ledger might not exist yet
    }
  }

  // 3. Register Commands
  context.subscriptions.push(
    vscode.commands.registerCommand("debuggatha.reviewWorkspace", () => {
      executeReview("workspace", findingsProvider, intelligenceProvider);
    }),
    vscode.commands.registerCommand("debuggatha.reviewDiff", () => {
      executeReview("diff", findingsProvider, intelligenceProvider);
    }),
    vscode.commands.registerCommand("debuggatha.reviewActiveFile", () => {
      executeReview("file", findingsProvider, intelligenceProvider);
    }),
    vscode.commands.registerCommand("debuggatha.reviewSelection", () => {
      // For now fallback to file
      executeReview("file", findingsProvider, intelligenceProvider);
    }),
    vscode.commands.registerCommand("debuggatha.openFinding", async (finding: Finding) => {
      const uri = docProvider.registerFinding(finding);
      const doc = await vscode.workspace.openTextDocument(uri);
      await vscode.window.showTextDocument(doc, {
        preview: true,
        viewColumn: vscode.ViewColumn.Two,
      });
    }),
  );

  registerLifecycleCommands(context, findingsProvider);

  // 4. Configuration listeners
  context.subscriptions.push(
    onSettingsChanged(() => {
      updateStatusBarStatus("Ready");
    }),
  );
}

export function deactivate() {
  clearDiagnostics();
}
