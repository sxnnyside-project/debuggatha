import * as vscode from "vscode";

export interface ExtensionSettings {
  reviewDepth: "quick" | "full" | "architectural";
  defaultReviewPacks: string[];
  logLevel: "debug" | "info" | "warn" | "error";
  runtime: "mcp" | "local";
  transport: "stdio" | "sse";
}

export function getSettings(): ExtensionSettings {
  const config = vscode.workspace.getConfiguration("debuggatha");
  return {
    reviewDepth: config.get("reviewDepth") || "full",
    defaultReviewPacks: config.get("defaultReviewPacks") || [
      "debuggatha/typescript",
      "debuggatha/react",
      "debuggatha/owasp",
    ],
    logLevel: config.get("logLevel") || "info",
    runtime: config.get("runtime") || "mcp",
    transport: config.get("transport") || "stdio",
  };
}

export function onSettingsChanged(
  callback: (settings: ExtensionSettings) => void,
): vscode.Disposable {
  return vscode.workspace.onDidChangeConfiguration((e) => {
    if (e.affectsConfiguration("debuggatha")) {
      callback(getSettings());
    }
  });
}
