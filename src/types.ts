/**
 * Shared types for Debuggatha extension
 */

export type WitchPersonality = 'angry' | 'kind' | 'wise';

export type ActionType = 'report' | 'audit' | 'analysis';

export type LLMProvider = 'gemini' | 'claude' | 'openai' | 'grok';

export interface LLMProviderConfig {
    id: LLMProvider;
    label: string;
    models: LLMModelOption[];
    secretKey: string; // key in SecretStorage
}

export interface LLMModelOption {
    id: string;
    label: string;
}

export interface FileContext {
    path: string;
    relativePath: string;
    content: string;
    language: string;
    size: number;
}

export interface ActionRequest {
    action: ActionType;
    personality: WitchPersonality;
    files: FileContext[];
    followUp?: string;
}

export interface ChatMessage {
    role: 'user' | 'assistant';
    content: string;
    personality?: WitchPersonality;
    action?: ActionType;
    timestamp: number;
}

export interface WebviewMessage {
    type: string;
    [key: string]: any;
}

/** Messages sent from webview to extension */
export type InboundMessage =
    | { type: 'executeAction'; action: ActionType; personality: WitchPersonality }
    | { type: 'sendFollowUp'; text: string; personality: WitchPersonality }
    | { type: 'changePersonality'; personality: WitchPersonality }
    | { type: 'changeProvider'; provider: LLMProvider }
    | { type: 'changeModel'; model: string }
    | { type: 'pickFiles' }
    | { type: 'removeFile'; index: number }
    | { type: 'checkApiKey' }
    | { type: 'configureApiKey' }
    | { type: 'ready' };

/** Messages sent from extension to webview */
export type OutboundMessage =
    | { type: 'init'; personality: WitchPersonality; provider: LLMProvider; model: string; providers: LLMProviderConfig[]; files: string[] }
    | { type: 'assistantMessage'; message: ChatMessage }
    | { type: 'loading'; isLoading: boolean; label?: string }
    | { type: 'error'; message: string }
    | { type: 'filesSelected'; files: string[] }
    | { type: 'fileRemoved'; files: string[] }
    | { type: 'personalityChanged'; personality: WitchPersonality }
    | { type: 'providerChanged'; provider: LLMProvider; models: LLMModelOption[] }
    | { type: 'modelChanged'; model: string }
    | { type: 'apiKeyStatus'; hasKey: boolean; provider: LLMProvider }
    | { type: 'clearOutput' };
