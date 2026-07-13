import { budgetSourceUnits, chunkSourceUnit, estimateTokens } from "./context-budget.js";
import type { ContextBudget, PromptBudgetResult, SemanticSourceUnit } from "./types.js";

const RESPONSE_INSTRUCTIONS = `You are a senior code reviewer. Review the provided source excerpts and respond with ONLY a JSON object (no prose, no markdown fences) of the exact shape:
{
  "candidates": [
    {
      "title": "short finding title",
      "explanation": "why this finding exists, citing what you observed",
      "confidence": "high" | "medium" | "low",
      "severityHint": "critical" | "high" | "medium" | "low" | "informational",
      "evidence": [{ "file": "path/from/the/excerpts", "lines": { "start": 1, "end": 3 }, "excerpt": "the exact code" }],
      "recommendation": "a concrete suggested fix"
    }
  ]
}
Every candidate MUST cite at least one evidence entry pointing at the provided excerpts. If you find nothing, return {"candidates": []}. Never invent a file or line that isn't in the excerpts below.`;

export interface BuiltPrompt {
  prompt: string;
  budget: PromptBudgetResult;
}

/**
 * Builds the actual prompt text sent to a provider: fixed response-format
 * instructions (so `parseSemanticResponse` has something reliable to
 * parse) plus as many source excerpts as the context budget allows.
 * Oversized units are chunked (`chunkSourceUnit`) before packing, so a
 * budget-exceeding file contributes its first N lines instead of being
 * dropped outright — see `context-budget.ts`.
 */
export function buildSemanticPrompt(
  units: SemanticSourceUnit[],
  budget: ContextBudget,
  policyStatements: string[] = [],
): BuiltPrompt {
  const reservedForScaffold =
    estimateTokens(RESPONSE_INSTRUCTIONS) + estimateTokens(policyStatements.join("\n"));
  const effectiveBudget: ContextBudget = {
    contextWindow: budget.contextWindow,
    reservedTokens: budget.reservedTokens + reservedForScaffold,
  };

  const contextWindow = budget.contextWindow ?? 4096;
  const availableForUnits = Math.max(256, contextWindow - effectiveBudget.reservedTokens);
  const maxChunkTokens = Math.max(64, Math.floor(availableForUnits / 2));

  const expanded = units.flatMap((unit) =>
    estimateTokens(unit.content) > maxChunkTokens ? chunkSourceUnit(unit, maxChunkTokens) : [unit],
  );

  const result = budgetSourceUnits(expanded, effectiveBudget);

  const excerpts = result.included
    .map((unit) => `--- ${unit.file} ---\n${unit.content}`)
    .join("\n\n");

  const policySection =
    policyStatements.length > 0
      ? `Apply these repository-specific criteria in addition to general engineering judgment:\n${policyStatements.map((s) => `- ${s}`).join("\n")}\n\n`
      : "";

  const prompt = `${RESPONSE_INSTRUCTIONS}\n\n${policySection}Source excerpts:\n\n${excerpts}`;

  return { prompt, budget: result };
}
