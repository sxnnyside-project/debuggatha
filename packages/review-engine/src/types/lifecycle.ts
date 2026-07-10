/**
 * The complete, enforced lifecycle — "future implementations should
 * never invent lifecycle states" (see the epic). `LEGAL_TRANSITIONS` is
 * the single source of truth for what's allowed; nothing outside this
 * file should encode lifecycle logic.
 */
export const REVIEW_LIFECYCLE_STATUSES = [
  "requested",
  "prepared",
  "running",
  "completed",
  "failed",
  "cancelled",
] as const;
export type ReviewLifecycleStatus = (typeof REVIEW_LIFECYCLE_STATUSES)[number];

const LEGAL_TRANSITIONS: Record<ReviewLifecycleStatus, readonly ReviewLifecycleStatus[]> = {
  requested: ["prepared", "cancelled"],
  prepared: ["running", "failed", "cancelled"],
  running: ["completed", "failed", "cancelled"],
  completed: [],
  failed: [],
  cancelled: [],
};

export function canTransition(from: ReviewLifecycleStatus, to: ReviewLifecycleStatus): boolean {
  return LEGAL_TRANSITIONS[from].includes(to);
}

export function isTerminal(status: ReviewLifecycleStatus): boolean {
  return LEGAL_TRANSITIONS[status].length === 0;
}
