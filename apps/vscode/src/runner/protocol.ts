import type {
  AnalyzerOptions,
  AnalyzerRun,
  ReviewPipelineScope,
  SemanticRun,
} from "@debuggatha/engine";

/** A review as data, so it can cross into the process that runs it. */
export interface ReviewJob {
  rootDir: string;
  scope: ReviewPipelineScope;
  packIds: string[];
  analyzers: AnalyzerOptions;
  /** Also have a model on this machine read the changed code. Only ever set by an explicit command. */
  semantic?: { provider: "ollama" | "lmstudio"; model?: string; url?: string };
}

/** What a review reports back. The findings themselves are in the ledger the review wrote. */
export interface ReviewJobResult {
  findings: number;
  analyzers: AnalyzerRun[];
  semantic: SemanticRun | null;
  changes: { introduced: number; fixed: number; reopened: number };
  durationMs: number;
}

export type WorkerMessage =
  | { type: "done"; result: ReviewJobResult }
  | { type: "error"; message: string };

export interface ReviewRunner {
  /** Rejects with `ReviewCancelled` when `signal` aborts, after the work has been stopped. */
  run(job: ReviewJob, signal: AbortSignal): Promise<ReviewJobResult>;
}

export class ReviewCancelled extends Error {
  constructor() {
    super("The review was cancelled.");
    this.name = "ReviewCancelled";
  }
}
