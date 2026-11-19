import { Daemon } from '../daemon/api.js';
import { DaemonConnection } from '../daemon/connection.js';
import { initializeDiagnostics } from '../diagnostics/diagnostics.js';
import { Logger } from '../logging.js';
import { FindingDocProvider } from '../providers/findingDocProvider.js';
import { FindingsProvider } from '../providers/findingsProvider.js';
import { IntelligenceProvider } from '../providers/intelligenceProvider.js';
import { ReviewCoordinator } from '../review/coordinator.js';
import { Services } from '../services.js';
import { StatusBar } from '../status/statusBar.js';
import { CLI } from './fixtures.js';
import { mock, openWorkspace, resetMock } from './vscode-mock.js';

const started: DaemonConnection[] = [];

/** Ends every daemon the harnesses started. */
export async function stopHarnesses(): Promise<void> {
  await Promise.all(started.splice(0).map((connection) => connection.dispose()));
}

/**
 * The extension's services wired to the vscode stand-in and to a real daemon (the `debuggatha`
 * binary): what the command tests need, so they exercise the real engine and ledger without an
 * editor; `stopHarnesses` ends them after each test.
 */
export function harness(root?: string) {
  resetMock();
  if (root) openWorkspace(root);
  const connection = new DaemonConnection({
    rootDir: () => root,
    settings: () => ({
      cliPath: CLI,
      enabledAnalyzers: [],
      semantic: { provider: 'off' },
      trusted: true,
    }),
    client: { name: 'debuggatha-vscode-test', version: '0.0.0' },
  });
  started.push(connection);
  const daemon = new Daemon(connection);
  initializeDiagnostics({ subscriptions: [] } as never);
  const status = new StatusBar({ subscriptions: [] } as never);
  const logger = new Logger({ appendLine: (line) => mock.outputLines.push(line) }, () => 'debug');
  const services = new Services(
    logger,
    daemon,
    new ReviewCoordinator(daemon),
    new FindingsProvider(),
    new IntelligenceProvider(),
    new FindingDocProvider(),
    status,
  );
  return { services, status };
}

/** What the editor is showing: each file's diagnostics. */
export const editorDiagnostics = () => [
  ...(mock.collections.get('debuggatha')?.entries.values() ?? []),
];

export const statusItem = () => mock.statusBarItems[0];
