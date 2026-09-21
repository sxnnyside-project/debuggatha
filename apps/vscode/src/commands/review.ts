import {
  assemblePolicy,
  buildRepositoryContext,
  createReviewRequest,
  LocalGitProvider,
  resolveRequestedPackIds,
  toRepoRelative,
} from "@debuggatha/engine";
import * as vscode from "vscode";
import { getSettings } from "../config/settings.js";
import type { Origin } from "../review/coordinator.js";
import { planReview, type ReviewRequest, selectionDiff } from "../review/plan.js";
import type { ReviewJob } from "../runner/protocol.js";
import type { Services } from "../services.js";

export interface ReviewOptions {
  origin: Origin;
  /** Also have the model on this machine read the changed code. Only an explicit command sets it. */
  semantic?: boolean;
  /** Let `reviewDepth: architectural` widen this to the whole repository. A save never does. */
  widen?: boolean;
}

/**
 * Turns a request into a review and shows what happened. A person's review has a cancellable
 * notification; one the editor started on save is silent, and only the status bar shows it.
 */
export async function runReview(
  services: Services,
  request: ReviewRequest,
  options: ReviewOptions,
): Promise<void> {
  const rootDir = services.rootDir;
  if (!rootDir) {
    if (options.origin === "manual") vscode.window.showErrorMessage("No workspace folder open.");
    return;
  }

  const settings = getSettings();
  const trusted = vscode.workspace.isTrusted;
  const plan = planReview(request, settings, trusted, { widen: options.widen !== false });

  let semantic: ReviewJob["semantic"];
  if (options.semantic) {
    if (settings.semantic.provider === "off") {
      const choice = await vscode.window.showInformationMessage(
        "Choose a model on this machine (Ollama or LM Studio) to use the model review. Nothing is sent to it until you do.",
        "Open Settings",
      );
      if (choice === "Open Settings") {
        void vscode.commands.executeCommand("workbench.action.openSettings", "debuggatha.semantic");
      }
      return;
    }
    semantic = {
      provider: settings.semantic.provider,
      ...(settings.semantic.model ? { model: settings.semantic.model } : {}),
      ...(settings.semantic.url ? { url: settings.semantic.url } : {}),
    };
  }

  const job: ReviewJob = {
    rootDir,
    scope: plan.scope,
    packIds: settings.extraReviewPacks,
    analyzers: plan.analyzers,
    ...(semantic ? { semantic } : {}),
  };

  services.logger.info("Review started", {
    origin: options.origin,
    request: request.kind,
    scope: plan.scope.kind,
    depth: settings.reviewDepth,
    trusted,
    analyzers: plan.analyzers.mode,
    model: semantic?.provider ?? "none",
  });
  services.status.reviewing(options.origin);

  const outcome =
    options.origin === "manual"
      ? await vscode.window.withProgress(
          {
            location: vscode.ProgressLocation.Notification,
            title: "Debuggatha",
            cancellable: true,
          },
          async (progress, token) => {
            token.onCancellationRequested(() => services.coordinator.cancel());
            progress.report({ message: `Reviewing (${settings.reviewDepth})...` });
            // The repository's context is what the Intelligence view shows; it is cheap next to the review.
            refreshIntelligence(services, rootDir, plan.scope.kind, settings.extraReviewPacks);
            return services.coordinator.run(options.origin, job);
          },
        )
      : await vscode.window.withProgress(
          { location: vscode.ProgressLocation.Window, title: "Debuggatha" },
          () => services.coordinator.run(options.origin, job),
        );

  switch (outcome.status) {
    case "done": {
      const { result } = outcome;
      services.refreshFromLedger();
      services.logger.info("Review finished", {
        findings: result.findings,
        introduced: result.changes.introduced,
        fixed: result.changes.fixed,
        reopened: result.changes.reopened,
        durationMs: result.durationMs,
      });
      for (const run of result.analyzers) {
        if (run.status === "ran") {
          services.logger.info("Analyzer ran", {
            id: run.id,
            findings: run.findings,
            ms: run.durationMs,
          });
        } else if (run.status === "failed") {
          services.logger.error("Analyzer failed", { id: run.id, reason: run.reason });
        } else {
          services.logger.debug("Analyzer skipped", { id: run.id, reason: run.reason });
        }
      }
      for (const note of result.semantic?.notes ?? [])
        services.logger.info("Model review", { note });

      if (options.origin === "manual") {
        const failed = result.analyzers.filter((run) => run.status === "failed");
        const parts = [
          `Review complete: ${result.findings} findings`,
          result.changes.introduced + result.changes.fixed > 0
            ? `(${result.changes.introduced} new, ${result.changes.fixed} fixed)`
            : "",
          result.semantic ? `, ${result.semantic.detected} raised by the model` : "",
          failed.length > 0
            ? `. ${failed.map((run) => run.name).join(", ")} failed; see the log`
            : "",
          ".",
        ];
        void vscode.window.showInformationMessage(parts.join("").replace(" ,", ","));
        for (const note of plan.notes) void vscode.window.showInformationMessage(note);
      }
      return;
    }
    case "cancelled":
      services.logger.info("Review cancelled");
      services.status.ready();
      if (options.origin === "manual")
        void vscode.window.showInformationMessage("Review cancelled.");
      return;
    case "skipped":
      services.logger.debug("Save review skipped: a review is already running");
      return;
    case "failed": {
      services.logger.error("Review failed", { reason: outcome.error });
      services.status.failed(outcome.error);
      if (options.origin !== "manual") return;
      const action = await vscode.window.showErrorMessage(
        `Debuggatha review failed: ${outcome.error}`,
        "Show Log",
        "Retry",
      );
      if (action === "Show Log") void vscode.commands.executeCommand("debuggatha.showLog");
      else if (action === "Retry") void runReview(services, request, options);
    }
  }
}

function refreshIntelligence(
  services: Services,
  rootDir: string,
  scopeKind: "workspace" | "files" | "diff",
  packs: string[],
): void {
  try {
    const context = buildRepositoryContext(rootDir);
    const policy = assemblePolicy(
      context,
      createReviewRequest({
        scope:
          scopeKind === "workspace" ? { kind: "workspace" } : { kind: "diff", base: undefined },
        depth: "full",
        requestedPackIds: resolveRequestedPackIds(packs),
      }),
    );
    services.intelligence.refresh(context, policy);
  } catch (error) {
    services.logger.debug("Could not refresh Repository Intelligence", {
      reason: error instanceof Error ? error.message : String(error),
    });
  }
}

/* ------------------------------------------------------------- what the commands ask for */

export async function requestForDiff(rootDir: string): Promise<ReviewRequest> {
  return { kind: "diff", diff: (await new LocalGitProvider(rootDir).getDiff()) ?? "" };
}

export function requestForActiveFile(): ReviewRequest | undefined {
  const editor = vscode.window.activeTextEditor;
  return editor ? { kind: "file", path: editor.document.uri.fsPath } : undefined;
}

/** The selected lines, whole, as a change: only they are reviewed, at their real line numbers. */
export function requestForSelection(rootDir: string): ReviewRequest | undefined {
  const editor = vscode.window.activeTextEditor;
  if (!editor || editor.selection.isEmpty) return undefined;
  const { document, selection } = editor;
  const first = selection.start.line;
  // A selection that ends at the start of a line does not include that line.
  const last = selection.end.character === 0 ? selection.end.line - 1 : selection.end.line;
  const lines: string[] = [];
  for (let line = first; line <= Math.max(first, last); line++)
    lines.push(document.lineAt(line).text);
  return {
    kind: "selection",
    path: document.uri.fsPath,
    diff: selectionDiff(toRepoRelative(rootDir, document.uri.fsPath), first + 1, lines),
  };
}
