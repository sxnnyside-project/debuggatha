import * as vscode from 'vscode';
import * as path from 'path';
import { GeminiClient } from '../services/GeminiClient';
import { PromptBuilder } from '../services/PromptBuilder';

export type WitchPersonality = 'angry' | 'kind' | 'wise';

interface ChatMessage {
    role: 'user' | 'assistant';
    content: string;
    personality?: WitchPersonality;
    timestamp: number;
}

interface WebviewMessage {
    type: string;
    [key: string]: any;
}

/**
 * Manages the Debuggatha webview panel lifecycle and communication
 */
export class DebuggathaPanel {
    private static currentPanel: DebuggathaPanel | undefined;
    private static chatHistory: ChatMessage[] = [];
    
    private readonly panel: vscode.WebviewPanel | vscode.WebviewView;
    private readonly context: vscode.ExtensionContext;
    private readonly geminiClient: GeminiClient;
    private readonly promptBuilder: PromptBuilder;
    private disposables: vscode.Disposable[] = [];
    private currentPersonality: WitchPersonality = 'kind';

    private constructor(
        panel: vscode.WebviewPanel | vscode.WebviewView,
        context: vscode.ExtensionContext
    ) {
        this.panel = panel;
        this.context = context;
        this.geminiClient = new GeminiClient(context);
        this.promptBuilder = new PromptBuilder(context);

        // Load default personality from settings
        const config = vscode.workspace.getConfiguration('debuggatha');
        this.currentPersonality = config.get('defaultPersonality', 'kind') as WitchPersonality;

        // Set up webview content
        this.setupWebview();

        // Set up message listener
        this.setupMessageListener();

        // Handle panel disposal
        if ('onDidDispose' in panel) {
            panel.onDidDispose(() => this.dispose(), null, this.disposables);
        }
    }

    /**
     * Render panel in standalone window
     */
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
                    vscode.Uri.file(path.join(context.extensionPath, 'webview'))
                ]
            }
        );

        DebuggathaPanel.currentPanel = new DebuggathaPanel(panel, context);
    }

    /**
     * Render panel in sidebar view
     */
    public static renderInView(
        webviewView: vscode.WebviewView,
        context: vscode.ExtensionContext
    ): void {
        webviewView.webview.options = {
            enableScripts: true,
            localResourceRoots: [
                vscode.Uri.file(path.join(context.extensionPath, 'media')),
                vscode.Uri.file(path.join(context.extensionPath, 'webview'))
            ]
        };

        new DebuggathaPanel(webviewView, context);
    }

    /**
     * Clear chat history
     */
    public static clearHistory(): void {
        DebuggathaPanel.chatHistory = [];
        if (DebuggathaPanel.currentPanel) {
            DebuggathaPanel.currentPanel.postMessage({ type: 'clearChat' });
        }
    }

    private setupWebview(): void {
        const webview = this.panel.webview;
        webview.html = this.getWebviewContent();
        
        // Get current model from config
        const config = vscode.workspace.getConfiguration('debuggatha');
        const currentModel = config.get<string>('aiModel', 'gemini-pro');
        
        // Send initial state
        this.postMessage({
            type: 'init',
            personality: this.currentPersonality,
            model: currentModel,
            history: DebuggathaPanel.chatHistory
        });
    }

    private setupMessageListener(): void {
        this.panel.webview.onDidReceiveMessage(
            async (message: WebviewMessage) => {
                switch (message.type) {
                    case 'sendMessage':
                        await this.handleUserMessage(
                            message.text,
                            message.files || [],
                            message.personality
                        );
                        break;
                    
                    case 'changePersonality':
                        this.currentPersonality = message.personality;
                        this.postMessage({
                            type: 'personalityChanged',
                            personality: this.currentPersonality
                        });
                        break;
                    
                    case 'changeModel':
                        this.geminiClient.setModel(message.model);
                        this.postMessage({
                            type: 'modelChanged',
                            model: message.model
                        });
                        break;
                    
                    case 'pickFiles':
                        await this.handleWorkspaceFilePicker();
                        break;
                    
                    case 'checkApiKey':
                        await this.checkApiKey();
                        break;
                }
            },
            null,
            this.disposables
        );
    }

    private async handleUserMessage(
        userText: string,
        selectedFiles: string[],
        personality: WitchPersonality
    ): Promise<void> {
        try {
            // Add user message to history
            const userMessage: ChatMessage = {
                role: 'user',
                content: userText,
                timestamp: Date.now()
            };
            DebuggathaPanel.chatHistory.push(userMessage);

            // Send user message to UI
            this.postMessage({
                type: 'userMessage',
                message: userMessage
            });

            // Show loading state
            this.postMessage({ type: 'loading', isLoading: true });

            // Read selected files
            const fileContents = await this.readFiles(selectedFiles);

            // Build prompt
            const prompt = this.promptBuilder.build(
                personality,
                userText,
                fileContents
            );

            // Get AI response
            const response = await this.geminiClient.generateResponse(prompt);

            // Add assistant message to history
            const assistantMessage: ChatMessage = {
                role: 'assistant',
                content: response,
                personality: personality,
                timestamp: Date.now()
            };
            DebuggathaPanel.chatHistory.push(assistantMessage);

            // Send assistant message to UI
            this.postMessage({
                type: 'assistantMessage',
                message: assistantMessage
            });

        } catch (error) {
            const errorMessage = error instanceof Error ? error.message : 'Unknown error occurred';
            this.postMessage({
                type: 'error',
                message: errorMessage
            });
            vscode.window.showErrorMessage(`Debuggatha Error: ${errorMessage}`);
        } finally {
            this.postMessage({ type: 'loading', isLoading: false });
        }
    }

    private async readFiles(filePaths: string[]): Promise<Array<{ path: string, content: string, language: string }>> {
        const config = vscode.workspace.getConfiguration('debuggatha');
        const maxFileSize = config.get<number>('maxFileSize', 100000);
        const maxFiles = config.get<number>('maxFilesPerRequest', 5);
        
        const limitedFiles = filePaths.slice(0, maxFiles);
        const results: Array<{ path: string, content: string, language: string }> = [];

        for (const filePath of limitedFiles) {
            try {
                const uri = vscode.Uri.file(filePath);
                const fileContent = await vscode.workspace.fs.readFile(uri);
                
                if (fileContent.length > maxFileSize) {
                    results.push({
                        path: filePath,
                        content: `[File too large: ${fileContent.length} bytes. Max: ${maxFileSize} bytes]`,
                        language: this.getLanguageFromPath(filePath)
                    });
                } else {
                    results.push({
                        path: filePath,
                        content: Buffer.from(fileContent).toString('utf8'),
                        language: this.getLanguageFromPath(filePath)
                    });
                }
            } catch (error) {
                results.push({
                    path: filePath,
                    content: `[Error reading file: ${error}]`,
                    language: 'text'
                });
            }
        }

        return results;
    }

    private getLanguageFromPath(filePath: string): string {
        const ext = path.extname(filePath).toLowerCase();
        const languageMap: { [key: string]: string } = {
            '.ts': 'typescript',
            '.js': 'javascript',
            '.py': 'python',
            '.java': 'java',
            '.cpp': 'cpp',
            '.c': 'c',
            '.cs': 'csharp',
            '.go': 'go',
            '.rs': 'rust',
            '.rb': 'ruby',
            '.php': 'php',
            '.html': 'html',
            '.css': 'css',
            '.json': 'json',
            '.xml': 'xml',
            '.md': 'markdown'
        };
        return languageMap[ext] || 'text';
    }

    private async handleFilePicker(): Promise<void> {
        const files = await vscode.window.showOpenDialog({
            canSelectMany: true,
            canSelectFiles: true,
            canSelectFolders: false,
            openLabel: 'Select Files for Context'
        });

        if (files && files.length > 0) {
            const filePaths = files.map(file => file.fsPath);
            this.postMessage({
                type: 'filesSelected',
                files: filePaths
            });
        }
    }

    /**
     * Handle workspace file picker (VS Code workspace-based)
     */
    private async handleWorkspaceFilePicker(): Promise<void> {
        // Get all workspace files
        const files = await vscode.workspace.findFiles(
            '**/*',
            '**/node_modules/**,**/.git/**,**/dist/**,**/build/**,**/*.min.js',
            100 // Limit to 100 files for performance
        );

        if (files.length === 0) {
            vscode.window.showInformationMessage('No files found in workspace');
            return;
        }

        // Create quick pick items
        const items = files.map(file => ({
            label: vscode.workspace.asRelativePath(file),
            description: path.dirname(vscode.workspace.asRelativePath(file)),
            uri: file
        }));

        // Show multi-select quick pick
        const selected = await vscode.window.showQuickPick(items, {
            canPickMany: true,
            placeHolder: 'Select files to include as context',
            matchOnDescription: true
        });

        if (selected && selected.length > 0) {
            const config = vscode.workspace.getConfiguration('debuggatha');
            const maxFiles = config.get<number>('maxFilesPerRequest', 5);
            
            const limitedSelection = selected.slice(0, maxFiles);
            const filePaths = limitedSelection.map(item => item.uri.fsPath);
            
            this.postMessage({
                type: 'filesSelected',
                files: filePaths
            });

            if (selected.length > maxFiles) {
                vscode.window.showWarningMessage(
                    `Only the first ${maxFiles} files will be included (limit: debuggatha.maxFilesPerRequest)`
                );
            }
        }
    }

    private async checkApiKey(): Promise<void> {
        const hasKey = await this.geminiClient.hasApiKey();
        this.postMessage({
            type: 'apiKeyStatus',
            hasKey
        });
    }

    private postMessage(message: any): void {
        this.panel.webview.postMessage(message);
    }

    private getWebviewContent(): string {
        const webview = this.panel.webview;
        const styleUri = webview.asWebviewUri(
            vscode.Uri.file(path.join(this.context.extensionPath, 'webview', 'styles.css'))
        );
        const scriptUri = webview.asWebviewUri(
            vscode.Uri.file(path.join(this.context.extensionPath, 'webview', 'script.js'))
        );
        
        // Get witch icon URIs
        const angryWitchUri = webview.asWebviewUri(
            vscode.Uri.file(path.join(this.context.extensionPath, 'media', 'angry-witch.svg'))
        );
        const kindWitchUri = webview.asWebviewUri(
            vscode.Uri.file(path.join(this.context.extensionPath, 'media', 'kind-witch.svg'))
        );
        const wiseWitchUri = webview.asWebviewUri(
            vscode.Uri.file(path.join(this.context.extensionPath, 'media', 'wise-witch.svg'))
        );

        return `<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <meta http-equiv="Content-Security-Policy" content="default-src 'none'; style-src ${webview.cspSource} 'unsafe-inline'; script-src ${webview.cspSource}; img-src ${webview.cspSource} data:;">
    <link href="${styleUri}" rel="stylesheet">
    <title>Debuggatha</title>
</head>
<body>
    <div class="container">
        <header class="header">
            <div class="header-title">Debuggatha</div>
        </header>

        <div class="config-section">
            <div class="config-row">
                <label class="config-label">Model</label>
                <select id="model-select" class="config-select">
                    <option value="gemini-pro">Gemini Pro</option>
                    <option value="gemini-pro-vision">Gemini Pro Vision</option>
                    <option value="gemini-ultra">Gemini Ultra</option>
                </select>
            </div>
            <div class="config-row">
                <label class="config-label">Personality</label>
                <div class="personality-selector">
                    <button class="personality-btn" data-personality="angry" title="Direct & Critical">
                        <img src="${angryWitchUri}" alt="Direct">
                    </button>
                    <button class="personality-btn active" data-personality="kind" title="Supportive">
                        <img src="${kindWitchUri}" alt="Supportive">
                    </button>
                    <button class="personality-btn" data-personality="wise" title="Architectural">
                        <img src="${wiseWitchUri}" alt="Architectural">
                    </button>
                </div>
            </div>
        </div>

        <div id="chat-area" class="chat-area"></div>

        <div class="input-section">
            <div class="file-section">
                <button id="pick-files-btn" class="file-btn">
                    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                        <path d="M21.44 11.05l-9.19 9.19a6 6 0 0 1-8.49-8.49l9.19-9.19a4 4 0 0 1 5.66 5.66l-9.2 9.19a2 2 0 0 1-2.83-2.83l8.49-8.48"/>
                    </svg>
                    <span id="file-count"></span>
                </button>
                <div id="selected-files" class="selected-files-inline"></div>
            </div>
            <div class="input-row">
                <textarea 
                    id="user-input" 
                    placeholder="Describe the issue..."
                    rows="1"
                ></textarea>
                <button id="send-btn" class="send-btn">
                    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                        <line x1="22" y1="2" x2="11" y2="13"></line>
                        <polygon points="22 2 15 22 11 13 2 9 22 2"></polygon>
                    </svg>
                </button>
            </div>
        </div>

        <div id="api-key-warning" class="warning" style="display: none;">
            API key not configured. Run "Debuggatha: Set Gemini API Key" command.
        </div>
    </div>
    <script src="${scriptUri}"></script>
</body>
</html>`;
    }

    private dispose(): void {
        DebuggathaPanel.currentPanel = undefined;
        
        if ('dispose' in this.panel) {
            this.panel.dispose();
        }

        while (this.disposables.length) {
            const disposable = this.disposables.pop();
            if (disposable) {
                disposable.dispose();
            }
        }
    }
}
