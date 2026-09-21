import * as vscode from "vscode";
import { canSuppressInline, suppressionEdit } from "../findings/present.js";
import type { Services } from "../services.js";

/**
 * Accepts a finding in the code itself, with a reason that stays beside it. The reason is
 * required: a comment that says nothing about why is how accepted risks get forgotten.
 * `reason` is an argument so a caller (a test, a keybinding) can supply it without a prompt.
 */
export async function suppressInline(
  services: Services,
  id: string,
  reason?: string,
): Promise<boolean> {
  const entry = services.index.entry(id);
  const rootDir = services.rootDir;
  if (!entry || !rootDir) {
    vscode.window.showErrorMessage("That finding is no longer in the ledger.");
    return false;
  }
  const finding = entry.latestFinding;
  if (!canSuppressInline(finding)) {
    vscode.window.showInformationMessage(
      "A comment cannot silence this finding, because it comes from an external tool. Dismiss it instead.",
    );
    return false;
  }

  const text =
    reason ??
    (await vscode.window.showInputBox({
      title: "Accept this finding in code",
      prompt: "Why is this acceptable here? The reason is kept in a comment beside the code.",
      validateInput: (value) => (value.trim() ? undefined : "A reason is required."),
    }));
  if (!text?.trim()) return false;

  const [file] = [...services.index.files()].filter((candidate) =>
    services.index.inFile(candidate).some((indexed) => indexed.entry.id === id),
  );
  if (!file) return false;

  const document = await vscode.workspace.openTextDocument(vscode.Uri.file(file));
  const line = Math.max(0, (finding.locations[0]?.lines?.start ?? 1) - 1);
  if (line >= document.lineCount) return false;
  const { text: comment, line: at } = suppressionEdit(finding, document.lineAt(line).text, text);

  const edit = new vscode.WorkspaceEdit();
  edit.insert(document.uri, new vscode.Position(at, 0), comment);
  const applied = await vscode.workspace.applyEdit(edit);
  services.logger.info("Finding accepted in code", { id, applied });
  return applied;
}
