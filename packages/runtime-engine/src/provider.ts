import type {
  HealthStatus,
  ModelInfo,
  SemanticExecutionRequest,
  SemanticStreamEvent,
} from "./types.js";

/**
 * The common provider interface (Epic 13 "Provider Abstraction") — every
 * provider exposes capabilities, never its own transport details. A
 * Runtime Engine consumer (or Review Skill, indirectly) never imports
 * `providers/ollama.js` or `providers/lmstudio.js` directly; it only ever
 * sees this interface, satisfied by whatever's registered.
 *
 * `execute` is streaming-first (an `AsyncIterable`) — a caller that wants
 * a single complete result just drains the iterable
 * (`consumeToCompletion` in `stream.ts` does exactly that); there is no
 * separate non-streaming method to keep in sync.
 */
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
   * the underlying transport (fetch) — "avoid orphaned requests" (Epic
   * 13 "Cancellation"). A provider that receives an already-aborted
   * signal yields no events at all.
   */
  execute(request: SemanticExecutionRequest): AsyncIterable<SemanticStreamEvent>;

  /** The model's context window in tokens, when the provider can report it — undefined, never guessed, otherwise. */
  contextWindowFor(model: string): Promise<number | undefined>;
}
