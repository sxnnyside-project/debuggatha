import * as vscode from "vscode";
import { getSettings } from "../config/settings.js";

let statusBarItem: vscode.StatusBarItem;

export function initializeStatusBar(context: vscode.ExtensionContext) {
  statusBarItem = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Right, 100);
  context.subscriptions.push(statusBarItem);
  updateStatusBarStatus("Ready");
  statusBarItem.show();
}

export function updateStatusBarStatus(status: "Ready" | "Reviewing...") {
  const settings = getSettings();
  const mode = settings.runtime === "mcp" ? "MCP" : "Local";

  if (status === "Reviewing...") {
    statusBarItem.text = `$(sync~spin) Debuggatha (${mode}): Reviewing`;
    statusBarItem.tooltip = "Debuggatha is currently performing a review.";
  } else {
    statusBarItem.text = `$(check-all) Debuggatha (${mode})`;
    statusBarItem.tooltip = "Debuggatha is ready.";
  }
}
