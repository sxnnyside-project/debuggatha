/**
 * The extension's settings, read from VS Code's configuration with a fallback for every value.
 *
 * @packageDocumentation
 */

import * as vscode from 'vscode';
import type { Severity } from '../contracts/index.js';

/**
 * How much a review does:
 *   quick          the built-in detectors only
 *   full           also the external analyzers installed on this machine
 *   architectural  also the whole-repository architecture pass (dependency cycles, layer
 *                  violations, dead code), whatever the command was
 */
export type ReviewDepth = 'quick' | 'full' | 'architectural';
/** How much the output channel records. */
export type LogLevel = 'off' | 'error' | 'info' | 'debug';
/** The model runtime the person configured for the optional semantic pass. */
export type SemanticProviderName = 'off' | 'ollama' | 'lmstudio';

/** Every review depth, from the least to the most. */
export const REVIEW_DEPTHS: readonly ReviewDepth[] = ['quick', 'full', 'architectural'];
/** Every log level, from the least to the most. */
export const LOG_LEVELS: readonly LogLevel[] = ['off', 'error', 'info', 'debug'];
/** Every severity, from the least to the most serious. */
export const SEVERITY_LEVELS: readonly Severity[] = [
  'informational',
  'low',
  'medium',
  'high',
  'critical',
];

/** What the extension reads from the `debuggatha.*` settings. */
export interface ExtensionSettings {
  /** The `debuggatha` binary: empty means the one on the PATH. Machine scope. */
  cliPath: string | undefined;
  /** Review Packs applied on top of the ones detected for the stack. */
  extraReviewPacks: string[];
  /** How much a review does. */
  reviewDepth: ReviewDepth;
  /** What the output channel records. */
  logLevel: LogLevel;
  /** Whether saving a file reviews it. */
  reviewOnSave: boolean;
  /** Lowest severity shown as a squiggle and in Problems; the findings view lists everything. */
  minimumSeverity: Severity;
  /** Analyzers that run project code or use the network, which only the user's own settings can enable. */
  enabledAnalyzers: string[];
  /** The model on this machine for the semantic pass; `off` sends nothing anywhere. */
  semantic: { provider: SemanticProviderName; model: string | undefined; url: string | undefined };
}

function pick<T extends string>(value: unknown, allowed: readonly T[], fallback: T): T {
  return typeof value === 'string' && (allowed as readonly string[]).includes(value)
    ? (value as T)
    : fallback;
}

/** The current settings, each value checked against what it may be. */
export function getSettings(): ExtensionSettings {
  const config = vscode.workspace.getConfiguration('debuggatha');
  const text = (key: string) => {
    const value = config.get<string>(key);
    return value?.trim() ? value.trim() : undefined;
  };
  return {
    cliPath: text('cliPath'),
    extraReviewPacks: config.get<string[]>('extraReviewPacks') ?? [],
    reviewDepth: pick(config.get<string>('reviewDepth'), REVIEW_DEPTHS, 'full'),
    logLevel: pick(config.get<string>('logLevel'), LOG_LEVELS, 'info'),
    reviewOnSave: config.get<boolean>('reviewOnSave') === true,
    minimumSeverity: pick(config.get<string>('minimumSeverity'), SEVERITY_LEVELS, 'informational'),
    enabledAnalyzers: config.get<string[]>('enabledAnalyzers') ?? [],
    semantic: {
      provider: pick(config.get<string>('semantic.provider'), ['off', 'ollama', 'lmstudio'], 'off'),
      model: text('semantic.model'),
      url: text('semantic.url'),
    },
  };
}

/** Calls `callback` with the new settings when one under `debuggatha` changes. */
export function onSettingsChanged(
  callback: (settings: ExtensionSettings) => void,
): vscode.Disposable {
  return vscode.workspace.onDidChangeConfiguration((e) => {
    if (e.affectsConfiguration('debuggatha')) {
      callback(getSettings());
    }
  });
}
