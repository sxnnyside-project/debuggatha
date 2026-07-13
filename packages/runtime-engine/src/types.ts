/**
 * Runtime Engine domain model (Epic 13).
 *
 * The single most load-bearing decision here: a provider never hands raw
 * model text to the rest of Debuggatha. Every provider execution
 * resolves to a `SemanticReviewResult` — a normalized structure of
 * candidate findings, each with evidence, a confidence level, and an
 * explanation — never a free-text blob. Swapping Ollama for LM Studio
 * (or any future provider) is transparent to Review Skills and the
 * Review Engine specifically *because* neither ever sees a provider's
 * own output format; they only ever see this shape. Turning a
 * `SemanticFindingCandidate` into a real `Finding` (review-engine) is the
 * Review Skills' job, not this package's — this package doesn't know
 * what a `Finding` is, the same independence `@debuggatha/review-engine`
 * already has from `@debuggatha/knowledge-system`.
 */

export interface ModelInfo {
  id: string;
  provider: string;
  /** Undefined when the provider can't report it — never guessed (see "never opine without evidence"). */
  contextWindow: number | undefined;
}

export interface HealthStatus {
  available: boolean;
  detail: string;
}

/** Where the Runtime Engine, not review-engine's `Evidence`, expects candidate evidence to live — review-engine's `Evidence` union is the eventual conversion target, not something this package depends on. */
export interface SemanticEvidence {
  file: string;
  lines: { start: number; end: number } | undefined;
  excerpt: string;
}

export type SemanticConfidence = "high" | "medium" | "low";

/**
 * One candidate finding a semantic review produced — deliberately not
 * called `Finding`: it hasn't been through `createFinding`'s invariants
 * yet (a real `Finding` requires an id, a `Category`, a `Severity`; a
 * candidate has neither, only a severity *hint* a Review Skill maps to a
 * real `Severity`).
 */
export interface SemanticFindingCandidate {
  title: string;
  explanation: string;
  confidence: SemanticConfidence;
  /** A hint, not review-engine's `Severity` — mapping hints to `Severity` is a Review Skill's decision, not this package's. */
  severityHint: "critical" | "high" | "medium" | "low" | "informational" | undefined;
  evidence: SemanticEvidence[];
  recommendation: string | undefined;
}

export interface SemanticReviewResult {
  candidates: SemanticFindingCandidate[];
  /** Non-empty when the provider's response couldn't be parsed into candidates at all — never silently dropped. */
  parseWarnings: string[];
}

/** A source unit handed to the Runtime Engine — the same shape `@debuggatha/skills`' engine already uses, duplicated here (not imported) to keep this package independent of `@debuggatha/skills` (wrong dependency direction — skills depends on this package, never the reverse). */
export interface SemanticSourceUnit {
  file: string;
  content: string;
}

export interface ContextBudget {
  /** The model's context window, in tokens — undefined means unknown, and budgeting falls back to a conservative default (see `context-budget.ts`). */
  contextWindow: number | undefined;
  /** Tokens reserved for the prompt scaffolding (instructions, policy statements) and the response itself. */
  reservedTokens: number;
}

export interface PromptBudgetResult {
  included: SemanticSourceUnit[];
  /** Every unit left out for budget reasons — reported, never silently dropped ("do not silently truncate important information"). */
  excluded: { file: string; reason: string }[];
  estimatedPromptTokens: number;
}

export type SemanticStreamEvent =
  | { kind: "token"; text: string }
  | { kind: "result"; result: SemanticReviewResult }
  | { kind: "error"; message: string }
  | { kind: "done" };

export interface SemanticExecutionRequest {
  model: string;
  prompt: string;
  signal: AbortSignal | undefined;
}

/** Runtime Metadata (Epic 13 "Runtime Metadata") — surfaced alongside a result, not folded into it, so consumers can log/display it without it polluting `SemanticReviewResult`'s own shape. */
export interface RuntimeExecutionMetadata {
  provider: string;
  model: string;
  contextWindow: number | undefined;
  durationMs: number;
  estimatedPromptTokens: number;
}
