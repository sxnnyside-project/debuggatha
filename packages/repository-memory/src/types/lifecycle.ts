/**
 * Repository Memory's lifecycle (Epic 15 — a design decision the epic's
 * own author called out as important for the product's whole future):
 * memory is never a write-once drawer everything piles into forever. It
 * moves through states the same way `@debuggatha/findings-ledger`'s
 * `FindingLifecycleStatus` already does — `LEGAL_TRANSITIONS` is the
 * single source of truth, nothing else in this package encodes
 * transition logic.
 *
 *   detected -> suggested -> confirmed -> active -> deprecated -> archived
 *
 * - `detected`: a deterministic pass (or a future Context
 *   Intelligence/Review Skill signal) noticed something memory-worthy,
 *   but no one has looked at it yet.
 * - `suggested`: surfaced to a user for review — "Debuggatha thinks this
 *   is worth remembering."
 * - `confirmed`: a user said yes. Not yet influencing reviews — a
 *   confirmed item still needs to be activated (kept as a distinct step
 *   so "I agree this is true" and "start acting on this" aren't
 *   conflated).
 * - `active`: influences reviews (suppression, accepted-deviation
 *   matching, etc. — see `filter.ts`). Only `active` items are ever
 *   consulted when filtering findings — "never present inferred
 *   knowledge as confirmed fact."
 * - `deprecated`: no longer applied to new reviews, but kept (not yet
 *   archived) — e.g. the repository changed enough that a convention
 *   might no longer hold, flagged for re-confirmation rather than
 *   silently dropped.
 * - `archived`: terminal. Kept for audit/history, never consulted, never
 *   surfaced as active guidance again.
 *
 * `reject` (User Experience: "reject memory") sends a `suggested` item
 * straight to `archived` — a suggestion a user declines was never true,
 * so there's nothing to deprecate.
 */
export const MEMORY_LIFECYCLE_STATUSES = [
  "detected",
  "suggested",
  "confirmed",
  "active",
  "deprecated",
  "archived",
] as const;
export type MemoryLifecycleStatus = (typeof MEMORY_LIFECYCLE_STATUSES)[number];

const LEGAL_TRANSITIONS: Record<MemoryLifecycleStatus, readonly MemoryLifecycleStatus[]> = {
  detected: ["suggested", "archived"],
  suggested: ["confirmed", "archived"],
  confirmed: ["active", "archived"],
  active: ["deprecated", "archived"],
  deprecated: ["active", "archived"],
  archived: [],
};

export function canTransitionMemory(
  from: MemoryLifecycleStatus,
  to: MemoryLifecycleStatus,
): boolean {
  return LEGAL_TRANSITIONS[from].includes(to);
}

/** Only `active` memory is ever consulted when filtering findings (see `filter.ts`) — every other status is inert with respect to reviews. */
export function isActiveMemoryStatus(status: MemoryLifecycleStatus): boolean {
  return status === "active";
}
