import type { Severity } from "@debuggatha/engine";
import * as vscode from "vscode";

/**
 * How much a review does:
 *   quick          the built-in detectors only
 *   full           also the external analyzers installed on this machine
 *   architectural  also the whole-repository architecture pass (dependency cycles, layer
 *                  violations, dead code), whatever the command was
 */
export type ReviewDepth = "quick" | "full" | "architectural";
export type LogLevel = "off" | "error" | "info" | "debug";
export type SemanticProviderName = "off" | "ollama" | "lmstudio";

export const REVIEW_DEPTHS: readonly ReviewDepth[] = ["quick", "full", "architectural"];
export const LOG_LEVELS: readonly LogLevel[] = ["off", "error", "info", "debug"];
export const SEVERITY_LEVELS: readonly Severity[] = [
  "informational",
  "low",
  "medium",
  "high",
  "critical",
];

export interface ExtensionSettings {
  extraReviewPacks: string[];
  reviewDepth: ReviewDepth;
  logLevel: LogLevel;
  reviewOnSave: boolean;
  /** Lowest severity shown as a squiggle and in Problems; the findings view lists everything. */
  minimumSeverity: Severity;
  /** Analyzers that run project code or use the network, which only the user's own settings can enable. */
  enabledAnalyzers: string[];
  semantic: { provider: SemanticProviderName; model: string | undefined; url: string | undefined };
}

function pick<T extends string>(value: unknown, allowed: readonly T[], fallback: T): T {
  return typeof value === "string" && (allowed as readonly string[]).includes(value)
    ? (value as T)
    : fallback;
}

export function getSettings(): ExtensionSettings {
  const config = vscode.workspace.getConfiguration("debuggatha");
  const text = (key: string) => {
    const value = config.get<string>(key);
    return value?.trim() ? value.trim() : undefined;
  };
  return {
    extraReviewPacks: config.get<string[]>("extraReviewPacks") ?? [],
    reviewDepth: pick(config.get<string>("reviewDepth"), REVIEW_DEPTHS, "full"),
    logLevel: pick(config.get<string>("logLevel"), LOG_LEVELS, "info"),
    reviewOnSave: config.get<boolean>("reviewOnSave") === true,
    minimumSeverity: pick(config.get<string>("minimumSeverity"), SEVERITY_LEVELS, "informational"),
    enabledAnalyzers: config.get<string[]>("enabledAnalyzers") ?? [],
    semantic: {
      provider: pick(config.get<string>("semantic.provider"), ["off", "ollama", "lmstudio"], "off"),
      model: text("semantic.model"),
      url: text("semantic.url"),
    },
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
