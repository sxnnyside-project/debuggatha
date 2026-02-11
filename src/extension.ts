import * as vscode from 'vscode';
import { DebuggathaPanel } from './panels/DebuggathaPanel';
import { LLMClientManager, LLM_PROVIDERS } from './services/LLMClientManager';
import { LLMProvider } from './types';

/**
 * Extension activation
 */
export function activate(context: vscode.ExtensionContext) {
    console.log('Debuggatha extension is now active');

    const llm = new LLMClientManager(context);

    // ── Commands ─────────────────────────────────────────────

    // Open as standalone panel
    context.subscriptions.push(
        vscode.commands.registerCommand('debuggatha.openPanel', () => {
            DebuggathaPanel.render(context);
        }),
    );

    // Configure API key (provider picker → input box → SecretStorage)
    context.subscriptions.push(
        vscode.commands.registerCommand('debuggatha.configureApiKey', async () => {
            const items = LLM_PROVIDERS.map(p => ({ label: p.label, id: p.id }));
            const picked = await vscode.window.showQuickPick(items, {
                placeHolder: 'Select AI provider to configure',
            });
            if (picked) {
                await llm.promptForApiKey(picked.id as LLMProvider);
            }
        }),
    );

    // Clear output history
    context.subscriptions.push(
        vscode.commands.registerCommand('debuggatha.clearOutput', () => {
            DebuggathaPanel.clearHistory();
            vscode.window.showInformationMessage('Debuggatha output cleared.');
        }),
    );

    // ── Sidebar view provider ────────────────────────────────

    const provider = new DebuggathaViewProvider(context);
    context.subscriptions.push(
        vscode.window.registerWebviewViewProvider('debuggatha.chatView', provider),
    );
}

/**
 * Sidebar webview view provider
 */
class DebuggathaViewProvider implements vscode.WebviewViewProvider {
    constructor(private readonly context: vscode.ExtensionContext) {}

    public resolveWebviewView(
        webviewView: vscode.WebviewView,
        _ctx: vscode.WebviewViewResolveContext,
        _token: vscode.CancellationToken,
    ) {
        DebuggathaPanel.renderInView(webviewView, this.context);
    }
}

/**
 * Extension deactivation
 */
export function deactivate() {
    console.log('Debuggatha extension is now deactivated');
}
