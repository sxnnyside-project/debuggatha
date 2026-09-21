import { initializeDiagnostics } from "../diagnostics/diagnostics.js";
import { Logger } from "../logging.js";
import { FindingDocProvider } from "../providers/findingDocProvider.js";
import { FindingsProvider } from "../providers/findingsProvider.js";
import { IntelligenceProvider } from "../providers/intelligenceProvider.js";
import { ReviewCoordinator } from "../review/coordinator.js";
import { createInProcessRunner } from "../runner/process-runner.js";
import { Services } from "../services.js";
import { StatusBar } from "../status/statusBar.js";
import { mock, openWorkspace, resetMock } from "./vscode-mock.js";

/**
 * The extension's services wired to the vscode stand-in, with reviews running in-process: what
 * the command tests need, so they exercise the real engine and ledger without an editor.
 */
export function harness(root?: string) {
  resetMock();
  if (root) openWorkspace(root);
  initializeDiagnostics({ subscriptions: [] } as never);
  const status = new StatusBar({ subscriptions: [] } as never);
  const logger = new Logger({ appendLine: (line) => mock.outputLines.push(line) }, () => "debug");
  const services = new Services(
    logger,
    new ReviewCoordinator(createInProcessRunner()),
    new FindingsProvider(),
    new IntelligenceProvider(),
    new FindingDocProvider(),
    status,
  );
  return { services, status };
}

/** What the editor is showing: each file's diagnostics. */
export const editorDiagnostics = () => [
  ...(mock.collections.get("debuggatha")?.entries.values() ?? []),
];

export const statusItem = () => mock.statusBarItems[0];
