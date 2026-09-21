import * as vscode from "vscode";
import type { FindingIndex } from "../findings/index.js";
import { hoverMarkdown } from "../findings/present.js";

/** What a person sees when pointing at a squiggle: the rule, where the claim comes from, why, and the fix. */
export class FindingHoverProvider implements vscode.HoverProvider {
  constructor(private readonly index: FindingIndex) {}

  provideHover(document: vscode.TextDocument, position: vscode.Position): vscode.Hover | undefined {
    const here = this.index
      .onLine(document.uri.fsPath, position.line)
      // With columns, only the text the finding is about; without, anywhere on the line.
      .filter(
        ({ span }) =>
          span.endChar === undefined ||
          (position.character >= span.startChar && position.character <= span.endChar),
      );
    if (here.length === 0) return undefined;

    const contents = here.map(({ entry }) => {
      const markdown = new vscode.MarkdownString(hoverMarkdown(entry));
      // Only Debuggatha's own commands may run from a link in this text, which comes from a repository.
      markdown.isTrusted = {
        enabledCommands: [
          "debuggatha.openFindingById",
          "debuggatha.resolveFindingById",
          "debuggatha.dismissFindingById",
          "debuggatha.suppressInline",
        ],
      };
      return markdown;
    });
    return new vscode.Hover(contents);
  }
}
