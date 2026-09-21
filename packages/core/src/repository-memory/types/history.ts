import type { MemoryLifecycleStatus } from "./lifecycle.js";

/**
 * What triggered a transition — mirrors
 * `core/findings-ledger`'s `TransitionOrigin`. `"detection"` is a
 * deterministic pass creating/advancing an item automatically;
 * `"user"` is an explicit human action (confirm/reject/archive — see
 * `api.ts`) and is the only origin that can ever move an item into
 * `"confirmed"`, keeping "user-confirmed" confidence honest.
 */
export type MemoryOrigin =
  | { kind: "detection"; sessionId: string }
  | { kind: "user"; actor: string };

/** One immutable record of a state change — append-only, same discipline as the Findings Ledger's `HistoryEvent`. */
export interface MemoryHistoryEvent {
  timestamp: string;
  previousState: MemoryLifecycleStatus | undefined;
  newState: MemoryLifecycleStatus;
  origin: MemoryOrigin;
  comment: string | undefined;
}
