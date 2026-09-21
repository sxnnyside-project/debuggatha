import type { RepositoryContext, ReviewPolicy } from "@debuggatha/engine";
import * as vscode from "vscode";

export class IntelligenceProvider implements vscode.TreeDataProvider<IntelligenceTreeItem> {
  private _onDidChangeTreeData: vscode.EventEmitter<IntelligenceTreeItem | undefined | void> =
    new vscode.EventEmitter<IntelligenceTreeItem | undefined | void>();
  readonly onDidChangeTreeData: vscode.Event<IntelligenceTreeItem | undefined | void> =
    this._onDidChangeTreeData.event;

  private context?: RepositoryContext;
  private policy?: ReviewPolicy;

  refresh(context: RepositoryContext, policy: ReviewPolicy): void {
    this.context = context;
    this.policy = policy;
    this._onDidChangeTreeData.fire();
  }

  getTreeItem(element: IntelligenceTreeItem): vscode.TreeItem {
    return element;
  }

  getChildren(element?: IntelligenceTreeItem): Thenable<IntelligenceTreeItem[]> {
    if (!this.context || !this.policy) {
      return Promise.resolve([]);
    }

    if (!element) {
      // Root nodes
      return Promise.resolve([
        new IntelligenceTreeItem("Stack", vscode.TreeItemCollapsibleState.Expanded, "stack"),
        new IntelligenceTreeItem(
          "Documentation",
          vscode.TreeItemCollapsibleState.Collapsed,
          "docs",
        ),
        new IntelligenceTreeItem("Criteria", vscode.TreeItemCollapsibleState.Collapsed, "criteria"),
        new IntelligenceTreeItem(
          "Active Review Policy",
          vscode.TreeItemCollapsibleState.Collapsed,
          "policy",
        ),
      ]);
    }

    if (element.nodeId === "stack") {
      const allSignals = [
        ...this.context.stack.languages,
        ...this.context.stack.frameworks,
        ...this.context.stack.buildSystems,
      ];
      const items = allSignals.map(
        (s) => new IntelligenceTreeItem(s.value, vscode.TreeItemCollapsibleState.None),
      );
      return Promise.resolve(items);
    }

    if (element.nodeId === "docs") {
      const items = this.context.documentation.sources.map(
        (d) =>
          new IntelligenceTreeItem(d.kind, vscode.TreeItemCollapsibleState.None, undefined, d.file),
      );
      return Promise.resolve(items);
    }

    if (element.nodeId === "criteria") {
      const items = this.context.criteria.rules.map(
        (r) =>
          new IntelligenceTreeItem(
            r.id,
            vscode.TreeItemCollapsibleState.None,
            undefined,
            r.description.length > 50 ? `${r.description.slice(0, 50)}...` : r.description,
          ),
      );
      return Promise.resolve(items);
    }

    if (element.nodeId === "policy") {
      const items = this.policy.packRefs.map(
        (p) =>
          new IntelligenceTreeItem(
            p.id,
            vscode.TreeItemCollapsibleState.None,
            undefined,
            p.version,
          ),
      );
      return Promise.resolve(items);
    }

    return Promise.resolve([]);
  }
}

export class IntelligenceTreeItem extends vscode.TreeItem {
  constructor(
    labelStr: string,
    collapsibleStateVal: vscode.TreeItemCollapsibleState,
    public readonly nodeId?: string,
    public readonly descriptionStr?: string,
  ) {
    super(labelStr, collapsibleStateVal);
    if (nodeId) this.id = nodeId;
    if (descriptionStr) this.description = descriptionStr;

    if (nodeId === "stack") this.iconPath = new vscode.ThemeIcon("layers");
    if (nodeId === "docs") this.iconPath = new vscode.ThemeIcon("book");
    if (nodeId === "criteria") this.iconPath = new vscode.ThemeIcon("checklist");
    if (nodeId === "policy") this.iconPath = new vscode.ThemeIcon("shield");
  }
}
