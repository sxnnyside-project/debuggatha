/**
 * The semantic layer is optional and only ever adds opinions. A model may raise a
 * finding no analyzer can see, or say a finding looks like a false positive, but
 * it never closes, dismisses, or hides anything: its verdict is evidence attached
 * to a finding, and a person or a rule decides what to do with it.
 */

export interface CompletionRequest {
  system: string;
  user: string;
  maxTokens: number;
}

/** Anything that can answer a prompt: the host's sampling, a local Ollama, a local LM Studio. */
export interface SemanticProvider {
  /** Shown beside every opinion the model gave (`ollama`, `lmstudio`, `host`). */
  name: string;
  /** The model that answered, when the provider knows it. */
  model: string | undefined;
  complete(request: CompletionRequest): Promise<string>;
}

export interface SemanticOptions {
  provider: SemanticProvider;
  /**
   * Attach the model's verdict to the candidates the built-in detectors are unsure of. Off by
   * default: on the labeled set a 7B local model does not meet the bar (see `eval/`), so a
   * verdict is shown to a reader and never changes a finding.
   */
  verify?: boolean;
  /** Read changed code for what analyzers do not see. Defaults to true. */
  detect?: boolean;
  /** At most this many findings are verified, most severe first. */
  maxCandidates?: number;
  /** At most this many files are read for detection. */
  maxDetectFiles?: number;
  /** Lines of one file sent to the model. */
  maxLines?: number;
}

/** What the semantic pass did in one review, reported whether or not it found anything. */
export interface SemanticRun {
  provider: string;
  model: string | undefined;
  verified: number;
  confirmed: number;
  doubtful: number;
  unsure: number;
  detected: number;
  /** Claims the model made that did not match the code, and were dropped. */
  rejected: number;
  /** Calls that failed or answered with something unusable. */
  failed: number;
  notes: string[];
  durationMs: number;
}

export const DEFAULT_SEMANTIC_LIMITS = {
  maxCandidates: 10,
  maxDetectFiles: 3,
  maxLines: 250,
} as const;
