import * as vscode from 'vscode';
import * as path from 'path';
import * as fs from 'fs';

export type WitchPersonality = 'angry' | 'kind' | 'wise';

interface PersonalityConfig {
    system: string;
    tone: string;
    guidelines: string[];
}

interface FileContext {
    path: string;
    content: string;
    language: string;
}

/**
 * PromptBuilder constructs optimized prompts for Gemini
 * Injects personality, context, and instructions dynamically
 */
export class PromptBuilder {
    private readonly context: vscode.ExtensionContext;
    private personalityConfigs: Map<WitchPersonality, PersonalityConfig> = new Map();

    constructor(context: vscode.ExtensionContext) {
        this.context = context;
        this.loadPersonalities();
    }

    /**
     * Load personality configurations from prompts directory
     */
    private loadPersonalities(): void {
        const promptsPath = path.join(this.context.extensionPath, 'prompts');

        // Define default personalities in case files don't exist
        this.personalityConfigs.set('angry', {
            system: 'You are an ANGRY WITCH expert in software development and debugging.',
            tone: 'You are sarcastic, judgmental, and harsh. You mock bad practices and poor code quality, but you ALWAYS provide technically correct solutions. You use aggressive but controlled language. You\'re frustrated by obvious bugs and bad patterns.',
            guidelines: [
                'Be rude and sarcastic about code quality issues',
                'Mock obvious mistakes and bad practices',
                'Use phrases like "seriously?", "did you even try?", "this is a mess"',
                'Despite the harsh tone, ALWAYS provide the correct solution',
                'Point out what the developer should have known',
                'Express disappointment in poor coding practices'
            ]
        });

        this.personalityConfigs.set('kind', {
            system: 'You are a KIND WITCH expert in software development and debugging.',
            tone: 'You are empathetic, supportive, calm, and patient. You encourage the developer and explain things gently. You\'re optimized for beginners or stressful debugging sessions. You make people feel better about their code.',
            guidelines: [
                'Be gentle and understanding',
                'Encourage the developer with positive reinforcement',
                'Use phrases like "don\'t worry", "it happens to everyone", "you\'re doing great"',
                'Explain concepts patiently without judgment',
                'Celebrate small wins and progress',
                'Provide reassurance that bugs are normal'
            ]
        });

        this.personalityConfigs.set('wise', {
            system: 'You are a WISE WITCH expert in software development and debugging.',
            tone: 'You are serious, precise, and professional. You speak like a senior software architect with decades of experience. You focus on best practices, performance, maintainability, and long-term architectural concerns.',
            guidelines: [
                'Use technical and professional language',
                'Focus on architectural patterns and best practices',
                'Discuss performance implications and scalability',
                'Consider long-term maintainability',
                'Reference industry standards and proven patterns',
                'Think about edge cases and system design'
            ]
        });

        // Try to load custom configurations if they exist
        try {
            const configPath = path.join(promptsPath, 'personalities.json');
            if (fs.existsSync(configPath)) {
                const configData = fs.readFileSync(configPath, 'utf8');
                const customConfigs = JSON.parse(configData);
                
                // Override with custom configs if present
                if (customConfigs.angry) {
                    this.personalityConfigs.set('angry', customConfigs.angry);
                }
                if (customConfigs.kind) {
                    this.personalityConfigs.set('kind', customConfigs.kind);
                }
                if (customConfigs.wise) {
                    this.personalityConfigs.set('wise', customConfigs.wise);
                }
            }
        } catch (error) {
            console.warn('Could not load custom personality configs, using defaults:', error);
        }
    }

    /**
     * Build a complete prompt for the AI
     */
    public build(
        personality: WitchPersonality,
        userQuestion: string,
        fileContexts: FileContext[]
    ): string {
        const config = this.personalityConfigs.get(personality);
        if (!config) {
            throw new Error(`Unknown personality: ${personality}`);
        }

        const parts: string[] = [];

        // 1. System role and personality injection
        parts.push('=== SYSTEM ROLE ===');
        parts.push(config.system);
        parts.push('');
        parts.push('=== PERSONALITY & TONE ===');
        parts.push(config.tone);
        parts.push('');
        parts.push('=== BEHAVIORAL GUIDELINES ===');
        config.guidelines.forEach((guideline, index) => {
            parts.push(`${index + 1}. ${guideline}`);
        });
        parts.push('');

        // 2. Core instructions
        parts.push('=== YOUR MISSION ===');
        parts.push('You are a debugging assistant. For every question:');
        parts.push('1. DIAGNOSE the issue by analyzing the code and context');
        parts.push('2. IDENTIFY possible root causes');
        parts.push('3. SUGGEST concrete solutions with code examples');
        parts.push('4. PROVIDE best practices to prevent similar issues');
        parts.push('5. STAY IN CHARACTER at all times according to your personality');
        parts.push('');

        // 3. File context (if provided)
        if (fileContexts.length > 0) {
            parts.push('=== CODE CONTEXT ===');
            parts.push(`The developer has provided ${fileContexts.length} file(s) for context:\n`);
            
            fileContexts.forEach((file, index) => {
                parts.push(`--- File ${index + 1}: ${file.path} ---`);
                parts.push(`Language: ${file.language}`);
                parts.push('```' + file.language);
                parts.push(file.content);
                parts.push('```');
                parts.push('');
            });
        }

        // 4. User question
        parts.push('=== DEVELOPER\'S QUESTION ===');
        parts.push(userQuestion);
        parts.push('');

        // 5. Response format instructions
        parts.push('=== RESPONSE FORMAT ===');
        parts.push('Respond in Markdown format with:');
        parts.push('- Clear section headers');
        parts.push('- Code blocks with proper syntax highlighting');
        parts.push('- Bullet points for lists');
        parts.push('- Bold text for emphasis');
        parts.push('');
        parts.push('REMEMBER: Stay completely in character as the ' + this.getPersonalityName(personality) + ' throughout your entire response!');

        return parts.join('\n');
    }

    /**
     * Build a simple prompt without file context
     */
    public buildSimple(personality: WitchPersonality, userQuestion: string): string {
        return this.build(personality, userQuestion, []);
    }

    /**
     * Get human-readable personality name
     */
    private getPersonalityName(personality: WitchPersonality): string {
        const names: Record<WitchPersonality, string> = {
            angry: 'Angry Witch',
            kind: 'Kind Witch',
            wise: 'Wise Witch'
        };
        return names[personality];
    }

    /**
     * Validate and sanitize user input
     */
    public sanitizeInput(input: string): string {
        // Remove any potential prompt injection attempts
        let sanitized = input.trim();
        
        // Limit length
        const maxLength = 10000;
        if (sanitized.length > maxLength) {
            sanitized = sanitized.substring(0, maxLength) + '\n\n[Input truncated due to length]';
        }

        return sanitized;
    }

    /**
     * Get personality description for UI
     */
    public getPersonalityDescription(personality: WitchPersonality): string {
        const descriptions: Record<WitchPersonality, string> = {
            angry: 'Harsh but technically correct. Mocks bad practices.',
            kind: 'Empathetic and supportive. Great for beginners.',
            wise: 'Professional and architectural. Focuses on best practices.'
        };
        return descriptions[personality];
    }
}
