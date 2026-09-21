import * as vscode from "vscode";

type State =
  | { kind: "ready" }
  | { kind: "reviewing"; origin: "manual" | "save" }
  | { kind: "findings"; count: number }
  | { kind: "error"; message: string };

/**
 * One item that says what Debuggatha is doing, and does the obvious thing when clicked: show the
 * findings, or cancel the review that is running.
 */
export class StatusBar {
  private readonly item: vscode.StatusBarItem;
  private lastCount = 0;

  constructor(context: vscode.ExtensionContext) {
    this.item = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Right, 100);
    context.subscriptions.push(this.item);
    this.set({ kind: "ready" });
    this.item.show();
  }

  reviewing(origin: "manual" | "save"): void {
    this.set({ kind: "reviewing", origin });
  }

  /** After a review or a change to the ledger: how many findings the editor is showing. */
  showFindings(count: number): void {
    this.lastCount = count;
    this.set({ kind: "findings", count });
  }

  /** Back to showing the count, once whatever was running is over. */
  ready(): void {
    this.set({ kind: "findings", count: this.lastCount });
  }

  failed(message: string): void {
    this.set({ kind: "error", message });
  }

  private set(state: State): void {
    switch (state.kind) {
      case "reviewing":
        this.item.text = "$(sync~spin) Debuggatha";
        this.item.tooltip =
          state.origin === "save"
            ? "Reviewing the file you saved. Click to cancel."
            : "Reviewing. Click to cancel.";
        this.item.command = "debuggatha.cancelReview";
        break;
      case "findings":
        this.item.text =
          state.count > 0 ? `$(warning) Debuggatha: ${state.count}` : "$(check-all) Debuggatha";
        this.item.tooltip =
          state.count > 0
            ? `${state.count} open ${state.count === 1 ? "finding" : "findings"}. Click to show them.`
            : "No open findings. Click to open Debuggatha.";
        this.item.command = "workbench.view.extension.debuggatha-explorer";
        break;
      case "error":
        this.item.text = "$(error) Debuggatha";
        this.item.tooltip = `The last review failed: ${state.message}. Click to see the log.`;
        this.item.command = "debuggatha.showLog";
        break;
      default:
        this.item.text = "$(check-all) Debuggatha";
        this.item.tooltip = "Debuggatha is ready.";
        this.item.command = "workbench.view.extension.debuggatha-explorer";
    }
  }
}
