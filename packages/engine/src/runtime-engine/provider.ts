import type {
  HealthStatus,
  ModelInfo,
  SemanticExecutionRequest,
  SemanticStreamEvent,
} from "./types.js";

/** A local model runtime. Every provider streams; a caller wanting one answer drains the iterable. */
export interface Provider {
  readonly id: string;
  readonly displayName: string;

  /** Lists models this provider can currently serve — never guessed; a provider that can't be reached returns an empty list, not a fabricated one. */
  discoverModels(): Promise<ModelInfo[]>;

  /** Cheap reachability/liveness check — used by "automatic" runtime selection to skip an unreachable provider instead of failing the whole review. */
  healthCheck(): Promise<HealthStatus>;

  /**
   * Streams execution events for one prompt against one model.
   * Cancellation: the caller's `AbortSignal` (on `request`) must abort
   * the underlying transport (fetch). A provider that receives an already-aborted
   * signal yields no events at all.
   */
  execute(request: SemanticExecutionRequest): AsyncIterable<SemanticStreamEvent>;

  /** The model's context window in tokens, when the provider can report it — undefined, never guessed, otherwise. */
  contextWindowFor(model: string): Promise<number | undefined>;
}
