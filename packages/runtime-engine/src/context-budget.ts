import type { ContextBudget, PromptBudgetResult, SemanticSourceUnit } from "./types.js";

/** Conservative fallback when a provider can't report its context window — small enough to be safe for the smallest commonly-run local models. */
const DEFAULT_CONTEXT_WINDOW = 4096;

/**
 * Token estimation (Epic 13 "Context Windows"). A real tokenizer is
 * model-specific (and most local providers don't expose one over HTTP);
 * ~4 characters per token is the standard conservative heuristic for
 * English-and-code text, deliberately over- rather than under-estimating
 * so budgeting stays on the safe side of a provider's real limit.
 */
export function estimateTokens(text: string): number {
  return Math.ceil(text.length / 4);
}

/**
 * Prompt chunking (Epic 13 "Context Windows" — "prompt chunking"): splits
 * one oversized unit into several smaller ones by line ranges, so a
 * single large file doesn't get wholesale excluded by
 * `budgetSourceUnits` when most of it would actually fit. Each chunk's
 * `file` is annotated with its line range so a caller can still cite
 * exactly which part of the file a candidate's evidence came from.
 */
export function chunkSourceUnit(
  unit: SemanticSourceUnit,
  maxTokensPerChunk: number,
): SemanticSourceUnit[] {
  const lines = unit.content.split("\n");
  const maxCharsPerChunk = Math.max(1, maxTokensPerChunk * 4);
  const chunks: SemanticSourceUnit[] = [];

  let currentLines: string[] = [];
  let currentChars = 0;
  let startLine = 1;

  const flush = (endLine: number) => {
    if (currentLines.length === 0) return;
    chunks.push({
      file:
        startLine === endLine
          ? `${unit.file}:${startLine}`
          : `${unit.file}:${startLine}-${endLine}`,
      content: currentLines.join("\n"),
    });
    currentLines = [];
    currentChars = 0;
  };

  lines.forEach((line, index) => {
    const lineNumber = index + 1;
    if (currentLines.length > 0 && currentChars + line.length > maxCharsPerChunk) {
      flush(lineNumber - 1);
      startLine = lineNumber;
    }
    currentLines.push(line);
    currentChars += line.length + 1;
  });
  flush(lines.length);

  return chunks;
}

/**
 * Context budgeting: given a token budget and a set of source units
 * (already reduced by the deterministic stage — "the deterministic stage
 * should reduce the amount of repository context sent to the model"),
 * greedily includes units (largest-context-value-first: here, simply
 * input order, since callers are expected to have already ranked/
 * filtered) until the budget is exhausted. Never silently drops a unit —
 * every excluded unit is named in `excluded` with a reason, so a caller
 * (or a Review Skill) can decide whether to chunk it further, warn the
 * user, or accept the gap.
 */
export function budgetSourceUnits(
  units: SemanticSourceUnit[],
  budget: ContextBudget,
): PromptBudgetResult {
  const contextWindow = budget.contextWindow ?? DEFAULT_CONTEXT_WINDOW;
  const availableTokens = Math.max(0, contextWindow - budget.reservedTokens);

  const included: SemanticSourceUnit[] = [];
  const excluded: PromptBudgetResult["excluded"] = [];
  let usedTokens = 0;

  for (const unit of units) {
    const unitTokens = estimateTokens(unit.content);
    if (usedTokens + unitTokens > availableTokens) {
      excluded.push({
        file: unit.file,
        reason:
          unitTokens > availableTokens
            ? `estimated ${unitTokens} tokens exceeds the entire available budget of ${availableTokens} tokens`
            : `would exceed the remaining budget (${availableTokens - usedTokens} tokens left, needs ~${unitTokens})`,
      });
      continue;
    }
    included.push(unit);
    usedTokens += unitTokens;
  }

  return { included, excluded, estimatedPromptTokens: usedTokens };
}
