import * as vscode from 'vscode';
import { LLMProvider, LLMProviderConfig, LLMModelOption } from '../types';

/**
 * Abstract LLM client interface.
 * All provider implementations must conform to this contract.
 */
export interface ILLMClient {
    readonly providerId: LLMProvider;
    generateResponse(prompt: string): Promise<string>;
    testConnection(): Promise<boolean>;
}

/**
 * Provider registry and configuration
 */
export const LLM_PROVIDERS: LLMProviderConfig[] = [
    {
        id: 'gemini',
        label: 'Google Gemini',
        models: [
            { id: 'gemini-2.0-flash', label: 'Gemini 2.0 Flash' },
            { id: 'gemini-2.0-pro', label: 'Gemini 2.0 Pro' },
            { id: 'gemini-1.5-pro', label: 'Gemini 1.5 Pro' },
            { id: 'gemini-1.5-flash', label: 'Gemini 1.5 Flash' },
        ],
        secretKey: 'debuggatha.apiKey.gemini',
    },
    {
        id: 'claude',
        label: 'Anthropic Claude',
        models: [
            { id: 'claude-sonnet-4-20250514', label: 'Claude Sonnet 4' },
            { id: 'claude-3-5-sonnet-20241022', label: 'Claude 3.5 Sonnet' },
            { id: 'claude-3-5-haiku-20241022', label: 'Claude 3.5 Haiku' },
            { id: 'claude-3-opus-20240229', label: 'Claude 3 Opus' },
        ],
        secretKey: 'debuggatha.apiKey.claude',
    },
    {
        id: 'openai',
        label: 'OpenAI / ChatGPT',
        models: [
            { id: 'gpt-4.1', label: 'GPT-4.1' },
            { id: 'gpt-4.1-mini', label: 'GPT-4.1 Mini' },
            { id: 'gpt-4o', label: 'GPT-4o' },
            { id: 'gpt-4o-mini', label: 'GPT-4o Mini' },
            { id: 'gpt-4-turbo', label: 'GPT-4 Turbo' },
        ],
        secretKey: 'debuggatha.apiKey.openai',
    },
    {
        id: 'grok',
        label: 'xAI Grok',
        models: [
            { id: 'grok-2', label: 'Grok 2' },
            { id: 'grok-2-mini', label: 'Grok 2 Mini' },
            { id: 'grok-beta', label: 'Grok Beta' },
        ],
        secretKey: 'debuggatha.apiKey.grok',
    },
];

/**
 * Unified LLM client manager.
 * Resolves the active provider, manages API keys via SecretStorage,
 * and dispatches generation requests to the correct backend.
 */
export class LLMClientManager {
    private context: vscode.ExtensionContext;
    private activeProvider: LLMProvider;
    private activeModel: string;
    private lastRequestTime: number = 0;
    private readonly MIN_REQUEST_INTERVAL = 1000;

    constructor(context: vscode.ExtensionContext) {
        this.context = context;
        const config = vscode.workspace.getConfiguration('debuggatha');
        this.activeProvider = config.get<LLMProvider>('provider', 'gemini');
        this.activeModel = config.get<string>('model', 'gemini-2.0-flash');
    }

    // ── Provider management ──────────────────────────────────

    public getActiveProvider(): LLMProvider {
        return this.activeProvider;
    }

    public getActiveModel(): string {
        return this.activeModel;
    }

    public getProviderConfig(provider?: LLMProvider): LLMProviderConfig | undefined {
        return LLM_PROVIDERS.find(p => p.id === (provider ?? this.activeProvider));
    }

    public getAllProviders(): LLMProviderConfig[] {
        return LLM_PROVIDERS;
    }

    public setProvider(provider: LLMProvider): void {
        this.activeProvider = provider;
        const providerConfig = this.getProviderConfig(provider);
        if (providerConfig && providerConfig.models.length > 0) {
            this.activeModel = providerConfig.models[0].id;
        }
    }

    public setModel(model: string): void {
        this.activeModel = model;
    }

    // ── API key management ───────────────────────────────────

    public async hasApiKey(provider?: LLMProvider): Promise<boolean> {
        const cfg = this.getProviderConfig(provider);
        if (!cfg) { return false; }
        const key = await this.context.secrets.get(cfg.secretKey);
        return !!key;
    }

    public async getApiKey(provider?: LLMProvider): Promise<string | undefined> {
        const cfg = this.getProviderConfig(provider);
        if (!cfg) { return undefined; }
        return this.context.secrets.get(cfg.secretKey);
    }

    public async setApiKey(provider: LLMProvider, apiKey: string): Promise<void> {
        const cfg = this.getProviderConfig(provider);
        if (!cfg) { throw new Error(`Unknown provider: ${provider}`); }
        await this.context.secrets.store(cfg.secretKey, apiKey);
    }

    public async promptForApiKey(provider?: LLMProvider): Promise<boolean> {
        const p = provider ?? this.activeProvider;
        const cfg = this.getProviderConfig(p);
        if (!cfg) { return false; }

        const apiKey = await vscode.window.showInputBox({
            prompt: `Enter your ${cfg.label} API Key`,
            password: true,
            placeHolder: 'Paste API key here…',
            ignoreFocusOut: true,
        });

        if (apiKey) {
            await this.setApiKey(p, apiKey);
            vscode.window.showInformationMessage(`${cfg.label} API key saved securely.`);
            return true;
        }
        return false;
    }

    // ── Generation ───────────────────────────────────────────

    public async generateResponse(prompt: string): Promise<string> {
        const apiKey = await this.getApiKey();
        if (!apiKey) {
            const cfg = this.getProviderConfig();
            throw new Error(
                `No API key configured for ${cfg?.label ?? this.activeProvider}. ` +
                `Use the command "Debuggatha: Configure API Key" to set one.`
            );
        }

        // Rate limiting
        const now = Date.now();
        const elapsed = now - this.lastRequestTime;
        if (elapsed < this.MIN_REQUEST_INTERVAL) {
            await new Promise(r => setTimeout(r, this.MIN_REQUEST_INTERVAL - elapsed));
        }
        this.lastRequestTime = Date.now();

        switch (this.activeProvider) {
            case 'gemini':
                return this.callGemini(apiKey, prompt);
            case 'claude':
                return this.callClaude(apiKey, prompt);
            case 'openai':
                return this.callOpenAI(apiKey, prompt);
            case 'grok':
                return this.callGrok(apiKey, prompt);
            default:
                throw new Error(`Unsupported provider: ${this.activeProvider}`);
        }
    }

    // ── Provider-specific HTTP calls ─────────────────────────

    private async callGemini(apiKey: string, prompt: string): Promise<string> {
        const url = `https://generativelanguage.googleapis.com/v1beta/models/${this.activeModel}:generateContent?key=${apiKey}`;
        const body = {
            contents: [{ parts: [{ text: prompt }] }],
            generationConfig: { temperature: 0.4, maxOutputTokens: 8192 },
        };

        const res = await fetch(url, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify(body),
        });

        if (!res.ok) {
            const err = await res.text();
            throw new Error(`Gemini API error (${res.status}): ${err}`);
        }

        const data: any = await res.json();
        const text = data?.candidates?.[0]?.content?.parts?.[0]?.text;
        if (!text) { throw new Error('Empty response from Gemini API'); }
        return text;
    }

    private async callClaude(apiKey: string, prompt: string): Promise<string> {
        const url = 'https://api.anthropic.com/v1/messages';
        const body = {
            model: this.activeModel,
            max_tokens: 8192,
            messages: [{ role: 'user', content: prompt }],
        };

        const res = await fetch(url, {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json',
                'x-api-key': apiKey,
                'anthropic-version': '2023-06-01',
            },
            body: JSON.stringify(body),
        });

        if (!res.ok) {
            const err = await res.text();
            throw new Error(`Claude API error (${res.status}): ${err}`);
        }

        const data: any = await res.json();
        const text = data?.content?.[0]?.text;
        if (!text) { throw new Error('Empty response from Claude API'); }
        return text;
    }

    private async callOpenAI(apiKey: string, prompt: string): Promise<string> {
        const url = 'https://api.openai.com/v1/chat/completions';
        const body = {
            model: this.activeModel,
            messages: [{ role: 'user', content: prompt }],
            temperature: 0.4,
            max_tokens: 8192,
        };

        const res = await fetch(url, {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json',
                'Authorization': `Bearer ${apiKey}`,
            },
            body: JSON.stringify(body),
        });

        if (!res.ok) {
            const err = await res.text();
            throw new Error(`OpenAI API error (${res.status}): ${err}`);
        }

        const data: any = await res.json();
        const text = data?.choices?.[0]?.message?.content;
        if (!text) { throw new Error('Empty response from OpenAI API'); }
        return text;
    }

    private async callGrok(apiKey: string, prompt: string): Promise<string> {
        // Grok uses an OpenAI-compatible API
        const url = 'https://api.x.ai/v1/chat/completions';
        const body = {
            model: this.activeModel,
            messages: [{ role: 'user', content: prompt }],
            temperature: 0.4,
            max_tokens: 8192,
        };

        const res = await fetch(url, {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json',
                'Authorization': `Bearer ${apiKey}`,
            },
            body: JSON.stringify(body),
        });

        if (!res.ok) {
            const err = await res.text();
            throw new Error(`Grok API error (${res.status}): ${err}`);
        }

        const data: any = await res.json();
        const text = data?.choices?.[0]?.message?.content;
        if (!text) { throw new Error('Empty response from Grok API'); }
        return text;
    }

    // ── Connection test ──────────────────────────────────────

    public async testConnection(): Promise<boolean> {
        try {
            const response = await this.generateResponse('Respond with exactly: OK');
            return response.trim().length > 0;
        } catch {
            return false;
        }
    }
}
