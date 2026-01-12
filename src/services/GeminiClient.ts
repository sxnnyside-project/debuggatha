import * as vscode from 'vscode';
import { GoogleGenerativeAI, GenerativeModel } from '@google/generative-ai';

/**
 * GeminiClient handles all interactions with Google's Gemini API
 * Includes error handling, rate limiting, and secure API key management
 */
export class GeminiClient {
    private readonly context: vscode.ExtensionContext;
    private genAI: GoogleGenerativeAI | null = null;
    private model: GenerativeModel | null = null;
    private currentModel: string = 'gemini-pro';
    private lastRequestTime: number = 0;
    private readonly MIN_REQUEST_INTERVAL = 1000; // 1 second between requests

    constructor(context: vscode.ExtensionContext) {
        this.context = context;
        this.loadModelPreference();
    }

    /**
     * Load model preference from settings
     */
    private loadModelPreference(): void {
        const config = vscode.workspace.getConfiguration('debuggatha');
        this.currentModel = config.get('aiModel', 'gemini-pro');
    }

    /**
     * Set the AI model to use
     */
    public setModel(modelName: string): void {
        this.currentModel = modelName;
        this.model = null; // Force reinitialization
    }

    /**
     * Get current model name
     */
    public getCurrentModel(): string {
        return this.currentModel;
    }

    /**
     * Check if API key is configured
     */
    public async hasApiKey(): Promise<boolean> {
        const apiKey = await this.context.secrets.get('debuggatha.geminiApiKey');
        return !!apiKey;
    }

    /**
     * Initialize the Gemini client with API key from secure storage
     */
    private async initialize(): Promise<void> {
        if (this.genAI && this.model) {
            return; // Already initialized
        }

        const apiKey = await this.context.secrets.get('debuggatha.geminiApiKey');
        
        if (!apiKey) {
            throw new Error(
                'Gemini API key not found. Please set it using the "Debuggatha: Set Gemini API Key" command.'
            );
        }

        try {
            this.genAI = new GoogleGenerativeAI(apiKey);
            this.model = this.genAI.getGenerativeModel({ model: this.currentModel });
        } catch (error) {
            throw new Error(`Failed to initialize Gemini client: ${error}`);
        }
    }

    /**
     * Generate a response from the Gemini API
     * Includes rate limiting and error handling
     */
    public async generateResponse(prompt: string): Promise<string> {
        await this.initialize();

        if (!this.model) {
            throw new Error('Gemini model not initialized');
        }

        // Rate limiting
        const now = Date.now();
        const timeSinceLastRequest = now - this.lastRequestTime;
        if (timeSinceLastRequest < this.MIN_REQUEST_INTERVAL) {
            const waitTime = this.MIN_REQUEST_INTERVAL - timeSinceLastRequest;
            await this.sleep(waitTime);
        }

        try {
            this.lastRequestTime = Date.now();

            const result = await this.model.generateContent(prompt);
            const response = result.response;
            const text = response.text();

            if (!text || text.trim().length === 0) {
                throw new Error('Received empty response from Gemini API');
            }

            return text;

        } catch (error) {
            return this.handleError(error);
        }
    }

    /**
     * Handle various API errors with user-friendly messages
     */
    private handleError(error: any): string {
        console.error('Gemini API Error:', error);

        // Check for specific error types
        if (error.message?.includes('API key')) {
            throw new Error('Invalid API key. Please check your Gemini API key and try again.');
        }

        if (error.message?.includes('quota') || error.message?.includes('rate limit')) {
            return `⚠️ **Rate Limit Exceeded**\n\nYou've hit the Gemini API rate limit. Please wait a moment and try again.\n\nTip: Consider upgrading your API plan for higher limits.`;
        }

        if (error.message?.includes('SAFETY')) {
            return `⚠️ **Content Safety Filter Triggered**\n\nThe Gemini API blocked this response due to safety settings. Try rephrasing your question or check your code for potentially sensitive content.`;
        }

        if (error.message?.includes('network') || error.message?.includes('ENOTFOUND')) {
            return `⚠️ **Network Error**\n\nCouldn't connect to Gemini API. Please check your internet connection and try again.`;
        }

        if (error.message?.includes('timeout')) {
            return `⚠️ **Request Timeout**\n\nThe request took too long. Try with less code or a simpler question.`;
        }

        // Generic error
        const errorMessage = error instanceof Error ? error.message : String(error);
        return `⚠️ **Error**\n\nSomething went wrong: ${errorMessage}\n\nPlease try again or check the extension logs for details.`;
    }

    /**
     * Utility function for rate limiting
     */
    private sleep(ms: number): Promise<void> {
        return new Promise(resolve => setTimeout(resolve, ms));
    }

    /**
     * Test the API connection
     */
    public async testConnection(): Promise<boolean> {
        try {
            await this.initialize();
            const testResponse = await this.generateResponse('Hello, respond with "OK" if you can hear me.');
            return testResponse.trim().length > 0;
        } catch (error) {
            console.error('Connection test failed:', error);
            return false;
        }
    }

    /**
     * Get current model information
     */
    public getModelInfo(): { name: string; provider: string } {
        return {
            name: this.currentModel,
            provider: 'Google Gemini'
        };
    }
}
