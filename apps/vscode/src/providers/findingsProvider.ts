import type { LedgerEntry } from "@debuggatha/core";
import * as vscode from "vscode";

export class FindingsProvider implements vscode.TreeDataProvider<FindingTreeItem> {
  private _onDidChangeTreeData: vscode.EventEmitter<FindingTreeItem | undefined | void> =
    new vscode.EventEmitter<FindingTreeItem | undefined | void>();
  readonly onDidChangeTreeData: vscode.Event<FindingTreeItem | undefined | void> =
    this._onDidChangeTreeData.event;

  private entries: LedgerEntry[] = [];
  private showResolved: boolean = false;

  refresh(entries: LedgerEntry[]): void {
    this.entries = entries;
    this._onDidChangeTreeData.fire();
  }

  toggleResolved(): void {
    this.showResolved = !this.showResolved;
    this._onDidChangeTreeData.fire();
  }

  private getVisibleEntries(): LedgerEntry[] {
    return this.showResolved ? this.entries : this.entries.filter((e) => e.status === "open");
  }

  getTreeItem(element: FindingTreeItem): vscode.TreeItem {
    return element;
  }

  getChildren(element?: FindingTreeItem): Thenable<FindingTreeItem[]> {
    const visible = this.getVisibleEntries();

    if (!element) {
      // Group by file
      const files = new Set(
        visible.map((e) => e.latestFinding.locations?.[0]?.file || "Workspace"),
      );
      const items = Array.from(files).map((file) => {
        return new FindingTreeItem(
          file,
          vscode.TreeItemCollapsibleState.Expanded,
          undefined,
          file === "Workspace" ? "project" : "file",
        );
      });
      return Promise.resolve(items);
    } else if (element.contextValue === "file" || element.contextValue === "project") {
      const fileEntries = visible.filter(
        (e) => (e.latestFinding.locations?.[0]?.file || "Workspace") === element.label,
      );
      const items = fileEntries.map((e) => {
        return new FindingTreeItem(
          e.latestFinding.title,
          vscode.TreeItemCollapsibleState.None,
          e,
          `finding-${e.status}`,
        );
      });
      return Promise.resolve(items);
    }

    return Promise.resolve([]);
  }
}

export class FindingTreeItem extends vscode.TreeItem {
  constructor(
    labelStr: string,
    collapsibleStateVal: vscode.TreeItemCollapsibleState,
    public readonly entry?: LedgerEntry,
    contextValueStr: string = "finding-open",
  ) {
    super(labelStr, collapsibleStateVal);
    this.contextValue = contextValueStr;
    const label = labelStr;
    const finding = entry?.latestFinding;
    this.tooltip = finding ? finding.title : label;

    if (finding && entry) {
      this.description = `${finding.severity} • ${entry.status}`;

      // Just use the description for status
      if (entry.status !== "open") {
        this.description += " (Resolved)";
      }

      this.iconPath = new vscode.ThemeIcon(this.getIconForSeverity(finding.severity));

      this.command = {
        command: "debuggatha.openFinding",
        title: "Open Finding",
        arguments: [finding],
      };
    } else {
      this.iconPath = new vscode.ThemeIcon(this.contextValue === "file" ? "file" : "project");
    }
  }

  private getIconForSeverity(severity: string): string {
    switch (severity) {
      case "critical":
        return "error";
      case "high":
        return "warning";
      case "medium":
        return "info";
      default:
        return "note";
    }
  }
}
