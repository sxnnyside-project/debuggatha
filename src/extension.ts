import * as vscode from 'vscode';
import { DebuggathaPanel } from './panels/DebuggathaPanel';

/**
 * Extension activation entry point
 * Called when the extension is activated
 */
export function activate(context: vscode.ExtensionContext) {
    console.log('Debuggatha extension is now active');

    // Register command to open the chat panel
    const openPanelCommand = vscode.commands.registerCommand(
        'debuggatha.openPanel',
        () => {
            DebuggathaPanel.render(context);
        }
    );

    // Register command to set API key
    const setApiKeyCommand = vscode.commands.registerCommand(
        'debuggatha.setApiKey',
        async () => {
            const apiKey = await vscode.window.showInputBox({
                prompt: 'Enter your Gemini API Key',
                password: true,
                placeHolder: 'AIza...',
                ignoreFocusOut: true
            });

            if (apiKey) {
                await context.secrets.store('debuggatha.geminiApiKey', apiKey);
                vscode.window.showInformationMessage('Gemini API Key saved securely!');
            }
        }
    );

    // Register command to clear chat history
    const clearChatCommand = vscode.commands.registerCommand(
        'debuggatha.clearChat',
        () => {
            DebuggathaPanel.clearHistory();
            vscode.window.showInformationMessage('Chat history cleared');
        }
    );

    // Register the webview view provider for the sidebar
    const provider = new DebuggathaViewProvider(context);
    context.subscriptions.push(
        vscode.window.registerWebviewViewProvider(
            'debuggatha.chatView',
            provider
        )
    );

    context.subscriptions.push(
        openPanelCommand,
        setApiKeyCommand,
        clearChatCommand
    );
}

/**
 * Webview View Provider for the sidebar
 */
class DebuggathaViewProvider implements vscode.WebviewViewProvider {
    constructor(private readonly context: vscode.ExtensionContext) {}

    public resolveWebviewView(
        webviewView: vscode.WebviewView,
        _context: vscode.WebviewViewResolveContext,
        _token: vscode.CancellationToken
    ) {
        // Initialize the webview through DebuggathaPanel
        DebuggathaPanel.renderInView(webviewView, this.context);
    }
}

/**
 * Extension deactivation
 */
export function deactivate() {
    console.log('Debuggatha extension is now deactivated');
}
