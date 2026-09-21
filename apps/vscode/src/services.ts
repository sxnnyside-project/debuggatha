import { loadLedger } from "@debuggatha/engine";
import * as vscode from "vscode";
import { getSettings } from "./config/settings.js";
import { updateDiagnostics } from "./diagnostics/diagnostics.js";
import { FindingIndex } from "./findings/index.js";
import type { Logger } from "./logging.js";
import type { FindingDocProvider } from "./providers/findingDocProvider.js";
import type { FindingsProvider } from "./providers/findingsProvider.js";
import type { IntelligenceProvider } from "./providers/intelligenceProvider.js";
import type { ReviewCoordinator } from "./review/coordinator.js";
import type { StatusBar } from "./status/statusBar.js";

/** What the commands and providers share: one of each, created when the extension activates. */
export class Services {
  readonly index = new FindingIndex();

  constructor(
    readonly logger: Logger,
    readonly coordinator: ReviewCoordinator,
    readonly findings: FindingsProvider,
    readonly intelligence: IntelligenceProvider,
    readonly docs: FindingDocProvider,
    readonly status: StatusBar,
  ) {}

  /** The workspace the extension reviews: the first folder. */
  get rootDir(): string | undefined {
    return vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
  }

  /**
   * Reads the ledger and brings the tree, the squiggles, and the status bar in line with it. The
   * ledger is the source of truth: a review, a lifecycle change, and a reload all end here.
   */
  refreshFromLedger(): void {
    const rootDir = this.rootDir;
    if (!rootDir) return;
    try {
      const { entries } = loadLedger(rootDir);
      this.findings.refresh([...entries]);
      this.index.rebuild(entries, rootDir, getSettings().minimumSeverity);
      updateDiagnostics(this.index);
      this.status.showFindings(countShown(this.index));
    } catch (error) {
      this.logger.error("Could not read the ledger", {
        reason: error instanceof Error ? error.message : String(error),
      });
    }
  }
}

function countShown(index: FindingIndex): number {
  let total = 0;
  for (const file of index.files()) total += index.inFile(file).length;
  return total;
}
