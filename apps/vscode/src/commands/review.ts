import {
  assemblePolicy,
  buildRepositoryContext,
  createReviewRequest,
  loadLedger,
  type ReviewPipelineScope,
  type ReviewScope,
  executeReview as runReviewPipeline,
} from "@debuggatha/core";
import * as vscode from "vscode";
import { getSettings } from "../config/settings.js";
import { updateDiagnostics } from "../diagnostics/diagnostics.js";
import type { FindingsProvider } from "../providers/findingsProvider.js";
import { VSCodeGitProvider } from "../providers/gitProvider.js";
import type { IntelligenceProvider } from "../providers/intelligenceProvider.js";
import { updateStatusBarStatus } from "../status/statusBar.js";

/**
 * Thin adapter over `@debuggatha/core`'s `executeReview` (Epic 11) — the
 * same pipeline `@debuggatha/mcp` and `@debuggatha/cli` run instead of a
 * third hand-rolled copy of the request/session/skill/ledger sequence.
 * This command still builds its own `RepositoryContext`/`ReviewPolicy`
 * once up front (`assemblePolicy` below) purely so the Repository
 * Intelligence tree view can refresh *before* the review runs — that's a
 * UI-ordering need, not a second orchestration path; `executeReview`
 * builds its own context/policy again internally as part of the one
 * shared pipeline.
 */
export async function executeReview(
  scope: "workspace" | "diff" | "file",
  findingsProvider: FindingsProvider,
  intelligenceProvider: IntelligenceProvider,
) {
  if (!vscode.workspace.workspaceFolders || vscode.workspace.workspaceFolders.length === 0) {
    vscode.window.showErrorMessage("No workspace folder open.");
    return;
  }

  const folders = vscode.workspace.workspaceFolders || [];
  const rootDir = folders[0]?.uri?.fsPath;
  if (!rootDir) {
    vscode.window.showErrorMessage("Workspace folder has no path.");
    return;
  }
  updateStatusBarStatus("Reviewing...");

  await vscode.window.withProgress(
    {
      location: vscode.ProgressLocation.Notification,
      title: "Debuggatha",
      cancellable: false,
    },
    async (progress) => {
      try {
        progress.report({ message: "Building Context..." });
        const gitProvider = new VSCodeGitProvider();
        const settings = getSettings();
        const depth = settings.reviewDepth as "quick" | "full" | "architectural";

        let pipelineScope: ReviewPipelineScope;
        let requestScope: ReviewScope;
        if (scope === "diff") {
          const diff = (await gitProvider.getDiff(rootDir)) ?? "";
          pipelineScope = { kind: "diff", base: undefined, diff };
          requestScope = { kind: "diff", base: undefined };
        } else if (scope === "workspace") {
          pipelineScope = { kind: "workspace" };
          requestScope = { kind: "workspace" };
        } else {
          const activeEditor = vscode.window.activeTextEditor;
          if (!activeEditor) {
            vscode.window.showErrorMessage("No active file to review.");
            return;
          }
          const paths = [activeEditor.document.uri.fsPath];
          pipelineScope = { kind: "files", paths };
          requestScope = { kind: "files", paths };
        }

        progress.report({ message: "Assembling Policy..." });
        const context = buildRepositoryContext(rootDir);
        const previewRequest = createReviewRequest({
          scope: requestScope,
          depth,
          requestedPackIds: settings.defaultReviewPacks,
        });
        const policy = assemblePolicy(context, previewRequest);
        intelligenceProvider.refresh(context, policy);

        progress.report({ message: `Analyzing ${scope}...` });
        const { result } = runReviewPipeline({
          rootDir,
          scope: pipelineScope,
          depth,
          packIds: settings.defaultReviewPacks,
          policyId: undefined,
          sourceName: "vscode",
        });

        const ledger = loadLedger(rootDir);
        findingsProvider.refresh(ledger.entries);

        const openFindings = ledger.entries
          .filter((e) => e.status === "open")
          .map((e) => e.latestFinding);
        updateDiagnostics(openFindings);

        vscode.window.showInformationMessage(
          `Debuggatha review complete. Found ${result.findings.length} findings.`,
        );
      } catch (err: unknown) {
        const message = err instanceof Error ? err.message : String(err);
        const action = await vscode.window.showErrorMessage(
          `Debuggatha review failed: ${message}`,
          "Open Settings",
          "Retry",
        );

        if (action === "Open Settings") {
          vscode.commands.executeCommand("workbench.action.openSettings", "debuggatha");
        } else if (action === "Retry") {
          executeReview(scope, findingsProvider, intelligenceProvider);
        }
      } finally {
        updateStatusBarStatus("Ready");
      }
    },
  );
}
