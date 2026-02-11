import * as vscode from 'vscode';
import * as path from 'path';
import {
    WitchPersonality, ActionType, LLMProvider,
    ChatMessage, InboundMessage,
} from '../types';
import { LLMClientManager, LLM_PROVIDERS } from '../services/LLMClientManager';
import { PromptBuilder } from '../services/PromptBuilder';
import { WorkspaceFileService } from '../services/WorkspaceFileService';

/**
 * Manages the Debuggatha webview panel lifecycle, message routing,
 * and orchestration of the three core actions (Report / Audit / Analysis).
 */
export class DebuggathaPanel {
    private static currentPanel: DebuggathaPanel | undefined;
    private static outputHistory: ChatMessage[] = [];

    private readonly panel: vscode.WebviewPanel | vscode.WebviewView;
    private readonly context: vscode.ExtensionContext;
    private readonly llm: LLMClientManager;
    private readonly promptBuilder: PromptBuilder;
    private readonly fileService: WorkspaceFileService;
    private disposables: vscode.Disposable[] = [];
    private currentPersonality: WitchPersonality = 'kind';

    // ── Construction ─────────────────────────────────────────

    private constructor(
        panel: vscode.WebviewPanel | vscode.WebviewView,
        context: vscode.ExtensionContext,
    ) {
        this.panel = panel;
        this.context = context;
        this.llm = new LLMClientManager(context);
        this.promptBuilder = new PromptBuilder(context);
        this.fileService = new WorkspaceFileService();

        const config = vscode.workspace.getConfiguration('debuggatha');
        this.currentPersonality = config.get('defaultPersonality', 'kind') as WitchPersonality;

        this.setupWebview();
        this.setupMessageListener();

        if ('onDidDispose' in panel) {
            panel.onDidDispose(() => this.dispose(), null, this.disposables);
        }
    }

    // ── Static entry points ──────────────────────────────────

    public static render(context: vscode.ExtensionContext): void {
        if (DebuggathaPanel.currentPanel) {
            if ('reveal' in DebuggathaPanel.currentPanel.panel) {
                DebuggathaPanel.currentPanel.panel.reveal(vscode.ViewColumn.Two);
            }
            return;
        }

        const panel = vscode.window.createWebviewPanel(
            'debuggatha',
            'Debuggatha',
            vscode.ViewColumn.Two,
            {
                enableScripts: true,
                retainContextWhenHidden: true,
                localResourceRoots: [
                    vscode.Uri.file(path.join(context.extensionPath, 'media')),
                    vscode.Uri.file(path.join(context.extensionPath, 'webview')),
                ],
            },
        );

        DebuggathaPanel.currentPanel = new DebuggathaPanel(panel, context);
    }

    public static renderInView(
        webviewView: vscode.WebviewView,
        context: vscode.ExtensionContext,
    ): void {
        webviewView.webview.options = {
            enableScripts: true,
            localResourceRoots: [
                vscode.Uri.file(path.join(context.extensionPath, 'media')),
                vscode.Uri.file(path.join(context.extensionPath, 'webview')),
            ],
        };
        new DebuggathaPanel(webviewView, context);
    }

    public static clearHistory(): void {
        DebuggathaPanel.outputHistory = [];
        DebuggathaPanel.currentPanel?.post({ type: 'clearOutput' });
    }

    public static getLLMManager(context: vscode.ExtensionContext): LLMClientManager | undefined {
        return DebuggathaPanel.currentPanel?.llm;
    }

    // ── Webview setup ────────────────────────────────────────

    private setupWebview(): void {
        const webview = this.panel.webview;
        webview.html = this.getHtml();
    }

    private setupMessageListener(): void {
        this.panel.webview.onDidReceiveMessage(
            async (msg: InboundMessage) => {
                switch (msg.type) {
                    case 'ready':
                        await this.sendInitState();
                        break;
                    case 'executeAction':
                        await this.handleAction(msg.action, msg.personality);
                        break;
                    case 'sendFollowUp':
                        await this.handleFollowUp(msg.text, msg.personality);
                        break;
                    case 'changePersonality':
                        this.currentPersonality = msg.personality;
                        this.post({ type: 'personalityChanged', personality: msg.personality });
                        break;
                    case 'changeProvider':
                        this.llm.setProvider(msg.provider);
                        const pCfg = this.llm.getProviderConfig(msg.provider);
                        this.post({
                            type: 'providerChanged',
                            provider: msg.provider,
                            models: pCfg?.models ?? [],
                        });
                        await this.checkAndReportApiKey();
                        break;
                    case 'changeModel':
                        this.llm.setModel(msg.model);
                        this.post({ type: 'modelChanged', model: msg.model });
                        break;
                    case 'pickFiles':
                        await this.handleFilePick();
                        break;
                    case 'removeFile':
                        this.fileService.removeFile(msg.index);
                        this.post({
                            type: 'filesSelected',
                            files: this.fileService.getSelectedRelativePaths(),
                        });
                        break;
                    case 'checkApiKey':
                        await this.checkAndReportApiKey();
                        break;
                    case 'configureApiKey':
                        const saved = await this.llm.promptForApiKey();
                        if (saved) {
                            await this.checkAndReportApiKey();
                        }
                        break;
                }
            },
            null,
            this.disposables,
        );
    }

    // ── State synchronization ────────────────────────────────

    private async sendInitState(): Promise<void> {
        this.post({
            type: 'init',
            personality: this.currentPersonality,
            provider: this.llm.getActiveProvider(),
            model: this.llm.getActiveModel(),
            providers: LLM_PROVIDERS,
            files: this.fileService.getSelectedRelativePaths(),
        });
        await this.checkAndReportApiKey();
    }

    private async checkAndReportApiKey(): Promise<void> {
        const hasKey = await this.llm.hasApiKey();
        this.post({
            type: 'apiKeyStatus',
            hasKey,
            provider: this.llm.getActiveProvider(),
        });
    }

    // ── Core action handler ──────────────────────────────────

    private async handleAction(action: ActionType, personality: WitchPersonality): Promise<void> {
        const actionLabels: Record<ActionType, string> = {
            report: 'Generating Report',
            audit: 'Generating Audit',
            analysis: 'Generating Analysis',
        };

        try {
            this.post({ type: 'loading', isLoading: true, label: actionLabels[action] });

            const fileContexts = await this.fileService.readSelectedFiles();
            const prompt = this.promptBuilder.buildActionPrompt(action, personality, fileContexts);
            const response = await this.llm.generateResponse(prompt);

            const msg: ChatMessage = {
                role: 'assistant',
                content: response,
                personality,
                action,
                timestamp: Date.now(),
            };
            DebuggathaPanel.outputHistory.push(msg);

            this.post({ type: 'assistantMessage', message: msg });
        } catch (err) {
            const errMsg = err instanceof Error ? err.message : String(err);
            this.post({ type: 'error', message: errMsg });
        } finally {
            this.post({ type: 'loading', isLoading: false });
        }
    }

    // ── Follow-up handler ────────────────────────────────────

    private async handleFollowUp(text: string, personality: WitchPersonality): Promise<void> {
        try {
            this.post({ type: 'loading', isLoading: true, label: 'Thinking…' });

            const sanitized = this.promptBuilder.sanitizeInput(text);
            const fileContexts = await this.fileService.readSelectedFiles();
            const prompt = this.promptBuilder.buildFollowUpPrompt(personality, sanitized, fileContexts);
            const response = await this.llm.generateResponse(prompt);

            const msg: ChatMessage = {
                role: 'assistant',
                content: response,
                personality,
                timestamp: Date.now(),
            };
            DebuggathaPanel.outputHistory.push(msg);

            this.post({ type: 'assistantMessage', message: msg });
        } catch (err) {
            const errMsg = err instanceof Error ? err.message : String(err);
            this.post({ type: 'error', message: errMsg });
        } finally {
            this.post({ type: 'loading', isLoading: false });
        }
    }

    // ── File picker ──────────────────────────────────────────

    private async handleFilePick(): Promise<void> {
        await this.fileService.pickFiles();
        this.post({
            type: 'filesSelected',
            files: this.fileService.getSelectedRelativePaths(),
        });
    }

    // ── Messaging ────────────────────────────────────────────

    private post(message: any): void {
        this.panel.webview.postMessage(message);
    }

    // ── HTML generation ──────────────────────────────────────

    private getHtml(): string {
        const webview = this.panel.webview;
        const uri = (rel: string) =>
            webview.asWebviewUri(vscode.Uri.file(path.join(this.context.extensionPath, rel)));

        const styleUri = uri('webview/styles.css');
        const scriptUri = uri('webview/script.js');

        return /*html*/ `<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <meta http-equiv="Content-Security-Policy"
          content="default-src 'none';
                   style-src ${webview.cspSource} 'unsafe-inline';
                   script-src ${webview.cspSource};
                   img-src ${webview.cspSource} data:;">
    <link href="${styleUri}" rel="stylesheet">
    <title>Debuggatha</title>
</head>
<body>
    <div class="container">

        <!-- ─── Header ───────────────────────────────── -->
        <header class="header">
            <span class="header-title">DEBUGGATHA</span>
            <div class="header-controls">
                <select id="provider-select" class="config-select" title="AI Provider"></select>
                <select id="model-select" class="config-select" title="Model"></select>
            </div>
        </header>

        <!-- ─── API key warning ──────────────────────── -->
        <div id="api-key-banner" class="banner banner--warning" style="display:none;">
            <span id="api-key-msg">API key not configured.</span>
            <button id="configure-key-btn" class="btn-link">Configure</button>
        </div>

        <!-- ─── Personality selector ─────────────────── -->
        <div class="personality-bar">
            <button class="personality-btn" data-personality="kind" title="Kind Witch — Patient &amp; encouraging">
                <svg class="personality-icon" width="16" height="16" viewBox="0 0 16 16" fill="none">
                    <path d="M8 1L2 3.5V7C2 10.5 4.5 13.5 8 15C11.5 13.5 14 10.5 14 7V3.5L8 1Z" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" stroke-linejoin="round" fill="none"/>
                    <path d="M8 11C8 11 5.5 9.5 5.5 7.75C5.5 6.8 6.2 6.25 7 6.25C7.5 6.25 7.8 6.5 8 6.75C8.2 6.5 8.5 6.25 9 6.25C9.8 6.25 10.5 6.8 10.5 7.75C10.5 9.5 8 11 8 11Z" stroke="currentColor" stroke-width="1.0" stroke-linecap="round" stroke-linejoin="round" fill="none"/>
                </svg>
                <span class="personality-label">Kind Witch</span>
            </button>
            <button class="personality-btn active" data-personality="wise" title="Technical Witch — Precise &amp; structured">
                <svg class="personality-icon" width="16" height="16" viewBox="0 0 16 16" fill="none">
                    <ellipse cx="8" cy="8" rx="6" ry="4" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" stroke-linejoin="round" fill="none"/>
                    <circle cx="8" cy="8" r="2" stroke="currentColor" stroke-width="1.2" fill="none"/>
                    <circle cx="8" cy="8" r="0.75" fill="currentColor"/>
                    <line x1="8" y1="2" x2="8" y2="4" stroke="currentColor" stroke-width="1.0" stroke-linecap="round"/>
                    <line x1="8" y1="12" x2="8" y2="14" stroke="currentColor" stroke-width="1.0" stroke-linecap="round"/>
                </svg>
                <span class="personality-label">Technical Witch</span>
            </button>
            <button class="personality-btn" data-personality="angry" title="Mean Witch — Sharp &amp; sarcastic">
                <svg class="personality-icon" width="16" height="16" viewBox="0 0 16 16" fill="none">
                    <polygon points="8,1 13.5,4.5 13.5,11.5 8,15 2.5,11.5 2.5,4.5" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" stroke-linejoin="round" fill="none"/>
                    <path d="M9.5 4L6.5 8.5H9L6.5 12" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round" fill="none"/>
                </svg>
                <span class="personality-label">Mean Witch</span>
            </button>
        </div>

        <!-- ─── Action buttons ───────────────────────── -->
        <div class="actions-bar">
            <button class="action-btn" data-action="report">
                <svg width="14" height="14" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round">
                    <rect x="3" y="1" width="10" height="14" rx="1"/>
                    <line x1="5.5" y1="4.5" x2="10.5" y2="4.5"/>
                    <line x1="5.5" y1="7" x2="10.5" y2="7"/>
                    <line x1="5.5" y1="9.5" x2="8.5" y2="9.5"/>
                </svg>
                Generate Report
            </button>
            <button class="action-btn" data-action="audit">
                <svg width="14" height="14" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round">
                    <path d="M8 1L2 3.5V7C2 10.5 4.5 13.5 8 15C11.5 13.5 14 10.5 14 7V3.5L8 1Z"/>
                    <polyline points="5.5 8 7 9.5 10.5 6"/>
                </svg>
                Generate Audit
            </button>
            <button class="action-btn" data-action="analysis">
                <svg width="14" height="14" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round">
                    <circle cx="8" cy="8" r="6"/>
                    <circle cx="8" cy="8" r="2"/>
                    <line x1="8" y1="2" x2="8" y2="4"/>
                    <line x1="8" y1="12" x2="8" y2="14"/>
                    <line x1="2" y1="8" x2="4" y2="8"/>
                    <line x1="12" y1="8" x2="14" y2="8"/>
                </svg>
                Generate Analysis
            </button>
        </div>

        <!-- ─── File selection ───────────────────────── -->
        <div class="file-bar">
            <button id="pick-files-btn" class="file-btn" title="Select workspace files">
                <svg width="14" height="14" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round">
                    <path d="M2 3C2 2.4 2.4 2 3 2H6.5L8 4H13C13.6 4 14 4.4 14 5V13C14 13.6 13.6 14 13 14H3C2.4 14 2 13.6 2 13V3Z"/>
                </svg>
                <span>Select Files</span>
                <span id="file-count" class="badge"></span>
            </button>
            <div id="selected-files" class="file-tags"></div>
        </div>

        <!-- ─── Output area ──────────────────────────── -->
        <div id="output-area" class="output-area">
            <div class="output-empty">
                <p>Select files and run an action to begin.</p>
            </div>
        </div>

        <!-- ─── Follow-up input ──────────────────────── -->
        <div class="input-section">
            <div class="input-row">
                <textarea id="user-input"
                    placeholder="Ask a follow-up question…"
                    rows="1"></textarea>
                <button id="send-btn" class="send-btn" title="Send">
                    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                        <line x1="22" y1="2" x2="11" y2="13"/>
                        <polygon points="22 2 15 22 11 13 2 9 22 2"/>
                    </svg>
                </button>
            </div>
        </div>
    </div>
    <script src="${scriptUri}"></script>
</body>
</html>`;
    }

    // ── Disposal ─────────────────────────────────────────────

    private dispose(): void {
        DebuggathaPanel.currentPanel = undefined;
        if ('dispose' in this.panel) {
            this.panel.dispose();
        }
        while (this.disposables.length) {
            this.disposables.pop()?.dispose();
        }
    }
}
