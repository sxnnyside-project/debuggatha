import * as vscode from 'vscode';
import * as path from 'path';
import * as fs from 'fs';
import { WitchPersonality, ActionType, FileContext } from '../types';

interface PersonalityConfig {
    system: string;
    tone: string;
    guidelines: string[];
}

/**
 * PromptBuilder constructs structured system prompts for the three core actions
 * (Report, Audit, Analysis) with personality-driven tone injection.
 */
export class PromptBuilder {
    private readonly context: vscode.ExtensionContext;
    private personalityConfigs: Map<WitchPersonality, PersonalityConfig> = new Map();

    constructor(context: vscode.ExtensionContext) {
        this.context = context;
        this.loadPersonalities();
    }

    // ── Personality loading ──────────────────────────────────

    private loadPersonalities(): void {
        // Defaults
        this.personalityConfigs.set('kind', {
            system: 'You are Debuggatha in KIND WITCH mode — a patient, encouraging code auditor.',
            tone: 'You are empathetic, supportive, and clear. You teach instead of shaming. You celebrate progress and explain issues gently, as if helping a colleague you care about.',
            guidelines: [
                'Use encouraging language: "nice start", "don\'t worry", "easy fix"',
                'Explain WHY something is a problem, not just that it is',
                'Offer concrete suggestions, not just criticism',
                'Frame issues as learning opportunities',
                'Be warm but still technically precise',
            ],
        });

        this.personalityConfigs.set('wise', {
            system: 'You are Debuggatha in TECHNICAL WITCH mode — a precise, professional code auditor.',
            tone: 'You are structured, authoritative, and concise. You treat the user as a senior engineer. You focus on architecture, performance, security, and maintainability with zero fluff.',
            guidelines: [
                'Use professional, technical language throughout',
                'Reference industry patterns, standards, and conventions',
                'Discuss performance implications and scalability concerns',
                'Consider edge cases, error handling, and system design',
                'Be direct and factual — no filler, no hand-holding',
            ],
        });

        this.personalityConfigs.set('angry', {
            system: 'You are Debuggatha in MEAN WITCH mode — a brutally honest code auditor.',
            tone: 'You are sharp, sarcastic, and unrelenting. You call out bad practices directly and with flair. You\'re frustrated by lazy code — but you ALWAYS provide real, actionable fixes alongside your roasts.',
            guidelines: [
                'Be sarcastic and cutting about bad patterns',
                'Use phrases like "seriously?", "who wrote this?", "this is chaos"',
                'Mock bad practices with specific technical reasoning',
                'Despite the harsh tone, ALWAYS provide correct solutions',
                'Make it entertaining but never cruel — the goal is improvement',
            ],
        });

        // Override from external config if available
        try {
            const configPath = path.join(this.context.extensionPath, 'prompts', 'personalities.json');
            if (fs.existsSync(configPath)) {
                const raw = fs.readFileSync(configPath, 'utf8');
                const custom = JSON.parse(raw);
                for (const key of ['angry', 'kind', 'wise'] as WitchPersonality[]) {
                    if (custom.personalities?.[key]) {
                        this.personalityConfigs.set(key, custom.personalities[key]);
                    }
                }
            }
        } catch (err) {
            console.warn('Could not load custom personality configs:', err);
        }
    }

    // ── Action descriptions ──────────────────────────────────

    private readonly actionInstructions: Record<ActionType, string> = {
        report: [
            '=== TASK: CODE QUALITY REPORT ===',
            'Generate a lightweight, high-level overview of the code quality.',
            '',
            'Structure your response as:',
            '## Summary',
            'A 2-3 sentence overall assessment.',
            '',
            '## Issues Found',
            'List each issue with:',
            '- **Severity** (Critical / Warning / Info)',
            '- **Location** (file and approximate area)',
            '- **Description** (what is wrong)',
            '',
            '## Quick Wins',
            'List 3-5 easy improvements the developer can make right now.',
            '',
            'Keep it concise. This is a quick feedback report, not a deep audit.',
        ].join('\n'),

        audit: [
            '=== TASK: DEEP CODE AUDIT ===',
            'Perform a thorough, structured technical review of the provided code.',
            '',
            'Structure your response with these sections:',
            '',
            '## Executive Summary',
            'Overall code health assessment (1-2 paragraphs).',
            '',
            '## Architecture',
            'Structural concerns, coupling, cohesion, separation of concerns.',
            '',
            '## Performance',
            'Bottlenecks, unnecessary allocations, algorithmic concerns.',
            '',
            '## Security',
            'Injection risks, exposure of secrets, authentication/authorization gaps.',
            '',
            '## Maintainability',
            'Readability, naming, duplication, testability, documentation gaps.',
            '',
            '## Recommendations',
            'Prioritized list of changes, ordered by impact.',
            '',
            'Be thorough. This is a deep professional audit.',
        ].join('\n'),

        analysis: [
            '=== TASK: CODE ANALYSIS ===',
            'Provide a balanced, contextual explanation of the patterns in this code.',
            '',
            'Structure your response as:',
            '',
            '## Overview',
            'What does this code do? What patterns does it use?',
            '',
            '## Design Reasoning',
            'Why were these patterns chosen? What tradeoffs were made?',
            '',
            '## What Works Well',
            'Identify strengths, good patterns, and solid decisions.',
            '',
            '## What Could Be Improved',
            'Identify weaknesses with context — explain WHY they matter.',
            '',
            '## Tradeoffs & Alternatives',
            'Discuss alternative approaches and when they would be preferable.',
            '',
            'Be balanced. Acknowledge both strengths and weaknesses.',
        ].join('\n'),
    };

    // ── Prompt construction ──────────────────────────────────

    /**
     * Build the full system prompt for a core action.
     */
    public buildActionPrompt(
        action: ActionType,
        personality: WitchPersonality,
        fileContexts: FileContext[],
    ): string {
        const pConfig = this.personalityConfigs.get(personality);
        if (!pConfig) { throw new Error(`Unknown personality: ${personality}`); }

        const parts: string[] = [];

        // Identity
        parts.push('=== SYSTEM ROLE ===');
        parts.push(pConfig.system);
        parts.push('');

        // Tone
        parts.push('=== PERSONALITY & TONE ===');
        parts.push(pConfig.tone);
        parts.push('');
        parts.push('=== BEHAVIORAL GUIDELINES ===');
        pConfig.guidelines.forEach((g, i) => parts.push(`${i + 1}. ${g}`));
        parts.push('');

        // Task
        parts.push(this.actionInstructions[action]);
        parts.push('');

        // Files
        if (fileContexts.length > 0) {
            parts.push('=== FILES PROVIDED ===');
            parts.push(`${fileContexts.length} file(s) from the workspace:\n`);
            for (const file of fileContexts) {
                parts.push(`--- ${file.relativePath} ---`);
                parts.push(`Language: ${file.language}`);
                parts.push('```' + file.language);
                parts.push(file.content);
                parts.push('```');
                parts.push('');
            }
        } else {
            parts.push('=== FILES PROVIDED ===');
            parts.push('No files were selected. Provide general guidance about the task.');
            parts.push('');
        }

        // Output format
        parts.push('=== RESPONSE FORMAT ===');
        parts.push('Respond in Markdown with clear headers, code blocks with syntax highlighting, and bullet points.');
        parts.push(`REMEMBER: Stay completely in character as the ${this.getPersonalityLabel(personality)} throughout.`);

        return parts.join('\n');
    }

    /**
     * Build a follow-up prompt (user types a question after an action).
     */
    public buildFollowUpPrompt(
        personality: WitchPersonality,
        userText: string,
        fileContexts: FileContext[],
    ): string {
        const pConfig = this.personalityConfigs.get(personality);
        if (!pConfig) { throw new Error(`Unknown personality: ${personality}`); }

        const parts: string[] = [];

        parts.push('=== SYSTEM ROLE ===');
        parts.push(pConfig.system);
        parts.push('');
        parts.push('=== PERSONALITY & TONE ===');
        parts.push(pConfig.tone);
        parts.push('');

        parts.push('=== TASK: FOLLOW-UP QUESTION ===');
        parts.push('The developer has a follow-up question about code they are working on.');
        parts.push('Answer clearly and stay in character.');
        parts.push('');

        if (fileContexts.length > 0) {
            parts.push('=== FILES PROVIDED ===');
            for (const file of fileContexts) {
                parts.push(`--- ${file.relativePath} ---`);
                parts.push('```' + file.language);
                parts.push(file.content);
                parts.push('```');
                parts.push('');
            }
        }

        parts.push('=== DEVELOPER\'S QUESTION ===');
        parts.push(userText);
        parts.push('');

        parts.push('=== RESPONSE FORMAT ===');
        parts.push('Respond in Markdown. Be concise but thorough.');

        return parts.join('\n');
    }

    private getPersonalityLabel(p: WitchPersonality): string {
        return { angry: 'Mean Witch', kind: 'Kind Witch', wise: 'Technical Witch' }[p];
    }

    /**
     * Sanitize user input for safety.
     */
    public sanitizeInput(input: string): string {
        let sanitized = input.trim();
        const maxLength = 10000;
        if (sanitized.length > maxLength) {
            sanitized = sanitized.substring(0, maxLength) + '\n[Input truncated]';
        }
        return sanitized;
    }
}
