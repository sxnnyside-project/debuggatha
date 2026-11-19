import * as vscode from 'vscode';
import { getSettings } from './config/settings.js';
import type { Daemon } from './daemon/api.js';
import { DaemonUnavailable } from './daemon/connection.js';
import { updateDiagnostics } from './diagnostics/diagnostics.js';
import { FindingIndex } from './findings/index.js';
import type { Logger } from './logging.js';
import type { FindingDocProvider } from './providers/findingDocProvider.js';
import type { FindingsProvider } from './providers/findingsProvider.js';
import type { IntelligenceProvider } from './providers/intelligenceProvider.js';
import type { ReviewCoordinator } from './review/coordinator.js';
import type { StatusBar } from './status/statusBar.js';

/** What the commands and providers share: one of each, created when the extension activates. */
export class Services {
  readonly index = new FindingIndex();

  constructor(
    readonly logger: Logger,
    readonly daemon: Daemon,
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
   * Reads the ledger from the daemon and brings the tree, the squiggles, and the status bar in
   * line with it. The ledger is the source of truth: a review, a lifecycle change, and a reload
   * all end here.
   */
  async refreshFromLedger(): Promise<void> {
    const rootDir = this.rootDir;
    if (!rootDir) return;
    try {
      const entries = await this.daemon.ledger();
      this.findings.refresh(entries);
      this.index.rebuild(entries, rootDir, getSettings().minimumSeverity);
      updateDiagnostics(this.index);
      this.status.showFindings(countShown(this.index));
    } catch (error) {
      const reason = error instanceof Error ? error.message : String(error);
      this.logger.error('Could not read the ledger', { reason });
      if (error instanceof DaemonUnavailable) await this.warnOnce(error);
    }
  }

  private warned = false;

  /** The binary is missing or does not fit: said once, with the setting that fixes it. */
  private async warnOnce(error: DaemonUnavailable): Promise<void> {
    this.status.failed(error.message);
    if (this.warned) return;
    this.warned = true;
    const choice = await vscode.window.showErrorMessage(
      `Debuggatha: ${error.message}`,
      ...(error.setting ? ['Open Settings'] : []),
    );
    if (choice === 'Open Settings') {
      void vscode.commands.executeCommand('workbench.action.openSettings', 'debuggatha.cliPath');
    }
  }
}

function countShown(index: FindingIndex): number {
  let total = 0;
  for (const file of index.files()) total += index.inFile(file).length;
  return total;
}
