import type { FindingLifecycleStatus } from "./lifecycle.js";

/**
 * What triggered a transition. Two kinds only, deliberately: an automated
 * detection pass (a review ran and either re-detected or stopped
 * detecting the finding) or a manual action (a human or a future adapter
 * — CLI, MCP, VS Code — took an explicit action). The Ledger stays
 * independent from those adapters (see CLAUDE.md non-negotiable and this
 * package's README) — `actor` is an opaque, caller-supplied string, never
 * an imported adapter identifier.
 */
export type TransitionOrigin =
  | { kind: "review"; sessionId: string }
  | { kind: "manual"; actor: string };

/**
 * One immutable record of a state change. History is append-only — see
 * `Ledger`'s own docs for why nothing in this package ever rewrites a
 * past `HistoryEvent`.
 */
export interface HistoryEvent {
  timestamp: string;
  previousState: FindingLifecycleStatus | undefined;
  newState: FindingLifecycleStatus;
  origin: TransitionOrigin;
  comment: string | undefined;
}
