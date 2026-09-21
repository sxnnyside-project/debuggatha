/**
 * The shapes a local model runtime (Ollama, LM Studio) speaks: model
 * discovery, health, and a streamed execution.
 */

export interface ModelInfo {
  id: string;
  provider: string;
  /** Undefined when the provider can't report it — never guessed. */
  contextWindow: number | undefined;
}

export interface HealthStatus {
  available: boolean;
  detail: string;
}

export interface SemanticReviewResult {
  candidates: SemanticFindingCandidate[];
  /** Non-empty when the response couldn't be parsed into candidates — never silently dropped. */
  parseWarnings: string[];
}

export interface SemanticEvidence {
  file: string;
  lines: { start: number; end: number } | undefined;
  excerpt: string;
}

export type SemanticConfidence = "high" | "medium" | "low";

export interface SemanticFindingCandidate {
  title: string;
  explanation: string;
  confidence: SemanticConfidence;
  severityHint: "critical" | "high" | "medium" | "low" | "informational" | undefined;
  evidence: SemanticEvidence[];
  recommendation: string | undefined;
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
  /** Instructions kept apart from the prompt, so text in the prompt cannot pass itself off as them. */
  system?: string | undefined;
  /** Ask the runtime for a JSON document, where it can enforce that. */
  json?: boolean | undefined;
  /** Sampling temperature; `0` for repeatable answers. */
  temperature?: number | undefined;
  /** Most tokens the answer may take. */
  maxTokens?: number | undefined;
}
