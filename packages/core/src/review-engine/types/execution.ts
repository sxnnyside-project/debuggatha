import type { ReviewSource } from "./source.js";

export interface ExecutionMetadata {
  source: ReviewSource | undefined;
  startedAt: string | undefined;
  completedAt: string | undefined;
  durationMs: number | undefined;
  error: string | undefined;
}

export function initialExecutionMetadata(): ExecutionMetadata {
  return {
    source: undefined,
    startedAt: undefined,
    completedAt: undefined,
    durationMs: undefined,
    error: undefined,
  };
}
