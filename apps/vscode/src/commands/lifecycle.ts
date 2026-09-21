import { getEntry, loadLedger, saveLedger, updateFindingStatus } from "@debuggatha/engine";
import * as vscode from "vscode";
import type { FindingsProvider, FindingTreeItem } from "../providers/findingsProvider.js";
import type { Services } from "../services.js";

type Target = "reopened" | "resolved" | "dismissed";

/**
 * Moving a finding through its lifecycle, from the tree (`item`) or from the editor (`ById`, in
 * a hover or the lightbulb). Both end at the ledger, then at `refreshFromLedger`, so the tree,
 * the squiggles, and the status bar cannot disagree.
 */
export function registerLifecycleCommands(
  context: vscode.ExtensionContext,
  services: Services,
  findingsProvider: FindingsProvider,
) {
  const fromTree = (target: Target) => async (item: FindingTreeItem) => {
    if (item.entry) await changeFindingStatus(services, item.entry.id, target);
  };
  const byId = (target: Target) => (id: string) => changeFindingStatus(services, id, target);

  context.subscriptions.push(
    vscode.commands.registerCommand("debuggatha.toggleResolvedFindings", () => {
      findingsProvider.toggleResolved();
    }),
    vscode.commands.registerCommand("debuggatha.resolveFinding", fromTree("resolved")),
    vscode.commands.registerCommand("debuggatha.dismissFinding", fromTree("dismissed")),
    vscode.commands.registerCommand("debuggatha.reopenFinding", fromTree("reopened")),
    vscode.commands.registerCommand("debuggatha.resolveFindingById", byId("resolved")),
    vscode.commands.registerCommand("debuggatha.dismissFindingById", byId("dismissed")),
  );
}

export async function changeFindingStatus(
  services: Services,
  id: string,
  status: Target,
): Promise<void> {
  const rootDir = services.rootDir;
  if (!rootDir) return;

  try {
    const ledger = loadLedger(rootDir);
    const entry = getEntry(ledger, id);
    if (!entry) {
      vscode.window.showErrorMessage("Finding not found in ledger.");
      return;
    }
    if (entry.status === status) return;

    // The ledger is immutable: the update returns a new ledger to persist.
    saveLedger(updateFindingStatus(ledger, id, status, { kind: "manual", actor: "vscode" }));
    services.logger.info("Finding status changed", { id, status });
    // Resolved and dismissed findings leave the editor's problem list.
    services.refreshFromLedger();
  } catch (err: unknown) {
    const message = err instanceof Error ? err.message : String(err);
    services.logger.error("Could not change a finding's status", { id, reason: message });
    vscode.window.showErrorMessage(`Failed to update finding status: ${message}`);
  }
}
