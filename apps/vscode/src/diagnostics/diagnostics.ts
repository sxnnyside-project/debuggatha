import type { Severity } from "@debuggatha/engine";
import * as vscode from "vscode";
import type { FindingIndex, IndexedFinding } from "../findings/index.js";
import { provenanceOf, ruleOf } from "../findings/present.js";

export const DIAGNOSTIC_SOURCE = "Debuggatha";

let diagnosticCollection: vscode.DiagnosticCollection;

export function initializeDiagnostics(context: vscode.ExtensionContext) {
  diagnosticCollection = vscode.languages.createDiagnosticCollection("debuggatha");
  context.subscriptions.push(diagnosticCollection);
}

const SEVERITY: Record<Severity, vscode.DiagnosticSeverity> = {
  critical: vscode.DiagnosticSeverity.Error,
  high: vscode.DiagnosticSeverity.Error,
  medium: vscode.DiagnosticSeverity.Warning,
  low: vscode.DiagnosticSeverity.Information,
  informational: vscode.DiagnosticSeverity.Hint,
};

/** The rule id is the diagnostic's code, and links to the rule's page when its tool has one. */
function codeOf(indexed: IndexedFinding): string | { value: string; target: vscode.Uri } {
  const finding = indexed.entry.latestFinding;
  const provenance = provenanceOf(finding);
  const value = ruleOf(finding);
  if (provenance.kind === "analyzer" && provenance.url) {
    return { value, target: vscode.Uri.parse(provenance.url) };
  }
  return value;
}

export function toDiagnostic(indexed: IndexedFinding): vscode.Diagnostic {
  const { span } = indexed;
  const range = new vscode.Range(span.line, span.startChar, span.line, span.endChar ?? 1_000_000);
  const finding = indexed.entry.latestFinding;
  const diagnostic = new vscode.Diagnostic(range, finding.title, SEVERITY[finding.severity]);
  diagnostic.source = DIAGNOSTIC_SOURCE;
  diagnostic.code = codeOf(indexed);
  return diagnostic;
}

/** Puts what the index holds into the Problems panel and the editor, replacing what was there. */
export function updateDiagnostics(index: FindingIndex) {
  diagnosticCollection.clear();
  for (const file of index.files()) {
    diagnosticCollection.set(vscode.Uri.file(file), index.inFile(file).map(toDiagnostic));
  }
}

export function clearDiagnostics() {
  if (diagnosticCollection) {
    diagnosticCollection.clear();
  }
}
