import { loadLedger, saveLedger, transitionFinding } from "@debuggatha/core";
import * as vscode from "vscode";
import { updateDiagnostics } from "../diagnostics/diagnostics.js";
import type { FindingsProvider, FindingTreeItem } from "../providers/findingsProvider.js";

export function registerLifecycleCommands(
  context: vscode.ExtensionContext,
  findingsProvider: FindingsProvider,
) {
  context.subscriptions.push(
    vscode.commands.registerCommand("debuggatha.toggleResolvedFindings", () => {
      findingsProvider.toggleResolved();
    }),

    vscode.commands.registerCommand("debuggatha.resolveFinding", async (item: FindingTreeItem) => {
      if (!item.entry) return;
      await changeFindingStatus(item.entry.id, "resolved", findingsProvider);
    }),

    vscode.commands.registerCommand("debuggatha.dismissFinding", async (item: FindingTreeItem) => {
      if (!item.entry) return;
      await changeFindingStatus(item.entry.id, "dismissed", findingsProvider);
    }),

    vscode.commands.registerCommand("debuggatha.reopenFinding", async (item: FindingTreeItem) => {
      if (!item.entry) return;
      await changeFindingStatus(item.entry.id, "open", findingsProvider);
    }),
  );
}

async function changeFindingStatus(
  id: string,
  status: "open" | "resolved" | "dismissed",
  findingsProvider: FindingsProvider,
) {
  const folders = vscode.workspace.workspaceFolders || [];
  const rootDir = folders[0]?.uri?.fsPath;
  if (!rootDir) return;

  try {
    const ledger = loadLedger(rootDir);

    const entryIndex = ledger.entries.findIndex((e) => e.id === id);
    if (entryIndex === -1) {
      vscode.window.showErrorMessage("Finding not found in ledger.");
      return;
    }

    const entry = ledger.entries[entryIndex];
    if (!entry) return;
    if (entry.status === status) return;

    // A prompt for comment could be added here in the future
    const newEntry = transitionFinding(entry, status, { kind: "manual", actor: "vscode" });
    ledger.entries[entryIndex] = newEntry;

    saveLedger(ledger);

    findingsProvider.refresh(ledger.entries);

    // Also update diagnostics to remove resolved ones from editor
    const openFindings = ledger.entries
      .filter((e) => e.status === "open")
      .map((e) => e.latestFinding);
    updateDiagnostics(openFindings);
  } catch (err: any) {
    vscode.window.showErrorMessage(`Failed to update finding status: ${err.message}`);
  }
}
