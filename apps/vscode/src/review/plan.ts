import type { AnalyzerOptions, ReviewPipelineScope } from "@debuggatha/engine";
import type { ExtensionSettings } from "../config/settings.js";

/** What the person asked for, before depth and trust decide how much of it happens. */
export type ReviewRequest =
  | { kind: "workspace" }
  | { kind: "diff"; diff: string }
  | { kind: "file"; path: string }
  | { kind: "selection"; diff: string; path: string };

export interface ReviewPlan {
  scope: ReviewPipelineScope;
  analyzers: AnalyzerOptions;
  /** True when depth widened the review beyond what was asked (architectural). */
  widened: boolean;
  /** Things worth telling the person: what was left out and why. */
  notes: string[];
}

/**
 * The one place `reviewDepth` and workspace trust become behavior.
 *
 * Trust comes first. An external analyzer is a program, and one that lives in the workspace
 * (`node_modules/.bin/biome`, `vendor/bin/phpstan`) is the workspace's code: running it in a
 * workspace nobody has trusted would run whatever a cloned repository put there. So an untrusted
 * workspace gets the built-in detectors only, whatever the depth.
 *
 * Analyzers that run project code or use the network run only if the person listed them in their
 * own settings (`debuggatha.enabledAnalyzers`, machine scope, so a repository's `.vscode` cannot).
 */
export function planReview(
  request: ReviewRequest,
  settings: Pick<ExtensionSettings, "reviewDepth" | "enabledAnalyzers">,
  trusted: boolean,
  { widen = true }: { widen?: boolean } = {},
): ReviewPlan {
  const notes: string[] = [];

  let analyzers: AnalyzerOptions;
  if (!trusted) {
    analyzers = { mode: "off" };
    notes.push(
      "This workspace is not trusted, so only the built-in detectors ran; external analyzers are off until you trust it.",
    );
  } else if (settings.reviewDepth === "quick") {
    analyzers = { mode: "off" };
  } else {
    analyzers = { mode: "auto", enable: settings.enabledAnalyzers };
  }

  const widened = widen && settings.reviewDepth === "architectural" && request.kind !== "workspace";
  if (widened) {
    notes.push(
      "reviewDepth is architectural, so the whole repository was reviewed, architecture included.",
    );
  }

  let scope: ReviewPipelineScope;
  if (widened || request.kind === "workspace") scope = { kind: "workspace" };
  else if (request.kind === "file") scope = { kind: "files", paths: [request.path] };
  else scope = { kind: "diff", base: undefined, diff: request.diff };

  return { scope, analyzers, widened, notes };
}

/**
 * A unified diff that adds exactly the given lines to a file, so a selection reviews like a
 * change: the engine reads only added lines and keeps their real line numbers.
 */
export function selectionDiff(file: string, startLine: number, lines: readonly string[]): string {
  const path = file.split("\\").join("/");
  return [
    `--- a/${path}`,
    `+++ b/${path}`,
    `@@ -0,0 +${startLine},${lines.length} @@`,
    ...lines.map((line) => `+${line}`),
    "",
  ].join("\n");
}
