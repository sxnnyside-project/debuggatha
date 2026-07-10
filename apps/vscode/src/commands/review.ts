import {
  assemblePolicy,
  buildRepositoryContext,
  createReviewRequest,
  createReviewResult,
  createReviewSession,
  type Finding,
  loadLedger,
  reviewArchitecture,
  reviewDiff,
  reviewFiles,
  type SyncScope,
  saveLedger,
  synchronizeReviewResult,
  transitionSession,
} from "@debuggatha/core";
import * as vscode from "vscode";
import { getSettings } from "../config/settings.js";
import { updateDiagnostics } from "../diagnostics/diagnostics.js";
import type { FindingsProvider } from "../providers/findingsProvider.js";
import { VSCodeGitProvider } from "../providers/gitProvider.js";
import type { IntelligenceProvider } from "../providers/intelligenceProvider.js";
import { updateStatusBarStatus } from "../status/statusBar.js";

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

        const context = await buildRepositoryContext(rootDir);

        progress.report({ message: "Assembling Policy..." });
        // 2. Assemble policy
        const settings = getSettings();
        const scopeObj =
          scope === "diff"
            ? { kind: "diff", base: undefined }
            : scope === "workspace"
              ? { kind: "workspace" }
              : {
                  kind: "files",
                  paths: [vscode.window.activeTextEditor?.document.uri.fsPath || ""],
                };
        const request = createReviewRequest({
          scope: scopeObj as any,
          depth: settings.reviewDepth as any,
          requestedPackIds: settings.defaultReviewPacks,
        });
        const policy = assemblePolicy(context, request);

        // Refresh intelligence tree
        intelligenceProvider.refresh(context, policy);

        // 3. Create Session
        let session = createReviewSession({ request, repositoryContext: context });
        session = transitionSession(session, "running");

        // 4. Run Review
        progress.report({ message: `Analyzing ${scope}...` });
        let findings: Finding[] = [];
        if (scope === "diff") {
          const diff = await gitProvider.getDiff(rootDir);
          if (diff) {
            findings = await reviewDiff(diff, policy);
          }
        } else if (scope === "workspace") {
          findings = await reviewArchitecture(rootDir, policy);
        } else if (scope === "file") {
          const activeEditor = vscode.window.activeTextEditor;
          if (!activeEditor) {
            vscode.window.showErrorMessage("No active file to review.");
            return;
          }
          const activeFilePath = activeEditor.document.uri.fsPath;
          findings = await reviewFiles([activeFilePath], policy);
        }

        session = transitionSession(session, "completed");

        // Determine sync scope
        const syncScope: SyncScope =
          scope === "workspace"
            ? { kind: "workspace" }
            : scope === "file"
              ? { kind: "files", files: [vscode.window.activeTextEditor!.document.uri.fsPath] }
              : { kind: "files", files: gitProvider.getUncommittedFiles(rootDir) };

        // 5. Synchronize Ledger
        const result = createReviewResult({ session, findings });
        let ledger = loadLedger(rootDir);
        const syncReport = synchronizeReviewResult(ledger, result, context, syncScope);
        ledger = syncReport.ledger;
        saveLedger(ledger);

        // 6. Update UI
        const ledgerEntries = ledger.entries;
        findingsProvider.refresh(ledgerEntries);

        // Diagnostics should only show open findings
        const openFindings = ledger.entries
          .filter((e) => e.status === "open")
          .map((e) => e.latestFinding);
        updateDiagnostics(openFindings);

        vscode.window.showInformationMessage(
          `Debuggatha review complete. Found ${findings.length} findings.`,
        );
      } catch (err: any) {
        const msg = `Debuggatha review failed: ${err.message}`;
        const action = await vscode.window.showErrorMessage(msg, "Open Settings", "Retry");

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
