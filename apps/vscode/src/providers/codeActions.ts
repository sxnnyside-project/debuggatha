import * as vscode from "vscode";
import { DIAGNOSTIC_SOURCE } from "../diagnostics/diagnostics.js";
import type { FindingIndex } from "../findings/index.js";
import { canSuppressInline, fixOf, quickFixEdit, ruleOf } from "../findings/present.js";

/**
 * The lightbulb on a Debuggatha squiggle: apply the exact fix when there is one, accept the
 * finding in code (with a reason), or mark it in the ledger. Every action is offered only if it
 * is safe to apply to the file as it is now.
 */
export class FindingCodeActionProvider implements vscode.CodeActionProvider {
  static readonly kinds = [vscode.CodeActionKind.QuickFix];

  constructor(private readonly index: FindingIndex) {}

  provideCodeActions(
    document: vscode.TextDocument,
    _range: vscode.Range,
    context: vscode.CodeActionContext,
  ): vscode.CodeAction[] {
    const actions: vscode.CodeAction[] = [];

    for (const diagnostic of context.diagnostics) {
      if (diagnostic.source !== DIAGNOSTIC_SOURCE) continue;
      const code =
        typeof diagnostic.code === "object"
          ? String(diagnostic.code.value)
          : String(diagnostic.code);
      const indexed = this.index
        .onLine(document.uri.fsPath, diagnostic.range.start.line)
        .find(({ entry }) => ruleOf(entry.latestFinding) === code);
      if (!indexed) continue;

      const { entry } = indexed;
      const finding = entry.latestFinding;

      const lineText =
        indexed.span.line < document.lineCount
          ? document.lineAt(indexed.span.line).text
          : undefined;
      const edit = quickFixEdit(finding, lineText);
      if (edit) {
        const fix = new vscode.CodeAction(
          `Debuggatha: ${fixOf(finding)?.summary ?? "apply the suggested fix"}`,
          vscode.CodeActionKind.QuickFix,
        );
        fix.diagnostics = [diagnostic];
        // A `safe` edit cannot change what the code does; one marked `review` needs a person to look.
        fix.isPreferred = edit.safety === "safe";
        fix.edit = new vscode.WorkspaceEdit();
        fix.edit.replace(
          document.uri,
          new vscode.Range(edit.line, edit.startChar, edit.line, edit.endChar),
          edit.replacement,
        );
        actions.push(fix);
      }

      if (canSuppressInline(finding)) {
        const suppress = new vscode.CodeAction(
          `Debuggatha: accept ${ruleOf(finding)} here, with a reason…`,
          vscode.CodeActionKind.QuickFix,
        );
        suppress.diagnostics = [diagnostic];
        suppress.command = {
          command: "debuggatha.suppressInline",
          title: "Accept in code",
          arguments: [entry.id],
        };
        actions.push(suppress);
      }

      for (const [title, command] of [
        ["Debuggatha: mark resolved", "debuggatha.resolveFindingById"],
        ["Debuggatha: dismiss", "debuggatha.dismissFindingById"],
        ["Debuggatha: open detail", "debuggatha.openFindingById"],
      ] as const) {
        const action = new vscode.CodeAction(title, vscode.CodeActionKind.QuickFix);
        action.diagnostics = [diagnostic];
        action.command = { command, title, arguments: [entry.id] };
        actions.push(action);
      }
    }
    return actions;
  }
}
