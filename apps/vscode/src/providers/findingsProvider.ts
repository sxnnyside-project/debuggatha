/**
 * The Findings tree view: the open findings grouped by file.
 *
 * @packageDocumentation
 */

import * as vscode from 'vscode';
import { isActiveFindingStatus, type LedgerEntry } from '../contracts/index.js';

/** Provides the findings tree. */
export class FindingsProvider implements vscode.TreeDataProvider<FindingTreeItem> {
  private _onDidChangeTreeData: vscode.EventEmitter<FindingTreeItem | undefined> =
    new vscode.EventEmitter<FindingTreeItem | undefined>();
  /** Fires when the tree changed. */
  readonly onDidChangeTreeData: vscode.Event<FindingTreeItem | undefined> =
    this._onDidChangeTreeData.event;

  private entries: LedgerEntry[] = [];
  private showResolved: boolean = false;

  /** Rebuilds the tree. */
  refresh(entries: LedgerEntry[]): void {
    this.entries = entries;
    this._onDidChangeTreeData.fire(undefined);
  }

  /** Shows or hides the resolved and dismissed findings. */
  toggleResolved(): void {
    this.showResolved = !this.showResolved;
    this._onDidChangeTreeData.fire(undefined);
  }

  private getVisibleEntries(): LedgerEntry[] {
    return this.showResolved
      ? this.entries
      : this.entries.filter((e) => isActiveFindingStatus(e.status));
  }

  /** The tree item to show for `element`. */
  getTreeItem(element: FindingTreeItem): vscode.TreeItem {
    return element;
  }

  /** The children of `element`, or the files when it is `undefined`. */
  getChildren(element?: FindingTreeItem): Thenable<FindingTreeItem[]> {
    const visible = this.getVisibleEntries();

    if (!element) {
      // Group by file
      const files = new Set(
        visible.map((e) => e.latestFinding.locations?.[0]?.file || 'Workspace'),
      );
      const items = Array.from(files).map((file) => {
        return new FindingTreeItem(
          file,
          vscode.TreeItemCollapsibleState.Expanded,
          undefined,
          file === 'Workspace' ? 'project' : 'file',
        );
      });
      return Promise.resolve(items);
    } else if (element.contextValue === 'file' || element.contextValue === 'project') {
      const fileEntries = visible.filter(
        (e) => (e.latestFinding.locations?.[0]?.file || 'Workspace') === element.label,
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

/** One finding in the tree. */
export class FindingTreeItem extends vscode.TreeItem {
  constructor(
    labelStr: string,
    collapsibleStateVal: vscode.TreeItemCollapsibleState,
    public readonly entry?: LedgerEntry,
    contextValueStr: string = 'finding-open',
  ) {
    super(labelStr, collapsibleStateVal);
    this.contextValue = contextValueStr;
    const label = labelStr;
    const finding = entry?.latestFinding;
    this.tooltip = finding ? finding.title : label;

    if (finding && entry) {
      this.description = `${finding.severity} • ${entry.status}`;

      this.iconPath = new vscode.ThemeIcon(this.getIconForSeverity(finding.severity));

      this.command = {
        command: 'debuggatha.openFinding',
        title: 'Open Finding',
        arguments: [finding],
      };
    } else {
      this.iconPath = new vscode.ThemeIcon(this.contextValue === 'file' ? 'file' : 'project');
    }
  }

  private getIconForSeverity(severity: string): string {
    switch (severity) {
      case 'critical':
        return 'error';
      case 'high':
        return 'warning';
      case 'medium':
        return 'info';
      default:
        return 'note';
    }
  }
}
