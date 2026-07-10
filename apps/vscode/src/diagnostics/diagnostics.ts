import type { Finding } from "@debuggatha/core";
import * as vscode from "vscode";

let diagnosticCollection: vscode.DiagnosticCollection;

export function initializeDiagnostics(context: vscode.ExtensionContext) {
  diagnosticCollection = vscode.languages.createDiagnosticCollection("debuggatha");
  context.subscriptions.push(diagnosticCollection);
}

export function updateDiagnostics(findings: Finding[]) {
  // Group findings by file URI
  const findingsByFile = new Map<string, Finding[]>();

  for (const finding of findings) {
    if (!finding.locations || finding.locations.length === 0) continue;
    const loc = finding.locations[0];
    if (!loc || !loc.file) continue;

    const file = loc.file;
    const fileFindings = findingsByFile.get(file) || [];
    fileFindings.push(finding);
    findingsByFile.set(file, fileFindings);
  }

  // Clear existing diagnostics before setting new ones
  diagnosticCollection.clear();

  // Create vscode diagnostics
  for (const [file, fileFindings] of findingsByFile.entries()) {
    const uri = vscode.Uri.file(file);
    const diagnostics = fileFindings.map((finding) => {
      // VS Code is 0-indexed for lines, finding locations are 1-indexed
      const loc = finding.locations && finding.locations[0];
      const line = Math.max(0, (loc?.lines?.start || 1) - 1);

      // Default to highlighting the whole line or just start of it
      const range = new vscode.Range(line, 0, line, 100);

      const severity =
        finding.severity === "critical" || finding.severity === "high"
          ? vscode.DiagnosticSeverity.Error
          : finding.severity === "medium"
            ? vscode.DiagnosticSeverity.Warning
            : vscode.DiagnosticSeverity.Information;

      const diagnostic = new vscode.Diagnostic(range, `[Debuggatha] ${finding.title}`, severity);

      diagnostic.source = "Debuggatha";
      if (finding.appliedPolicy) {
        diagnostic.code = finding.appliedPolicy.id;
      }

      return diagnostic;
    });

    diagnosticCollection.set(uri, diagnostics);
  }
}

export function clearDiagnostics() {
  if (diagnosticCollection) {
    diagnosticCollection.clear();
  }
}
