/**
 * The complete, enforced lifecycle (epic §3) — mirrors the same pattern
 * `core/review-engine`'s `ReviewLifecycleStatus` already
 * established: `LEGAL_TRANSITIONS` is the single source of truth, nothing
 * else in the codebase encodes transition logic.
 *
 * "Reopened" is a distinct state from "open", not an alias — it records
 * that a finding was previously closed (resolved or dismissed) and has
 * now reappeared, which is a meaningful fact for reporting ("how many
 * findings came back") that collapsing it into "open" would lose.
 */
export const FINDING_LIFECYCLE_STATUSES = [
  "open",
  "acknowledged",
  "resolved",
  "dismissed",
  "reopened",
] as const;
export type FindingLifecycleStatus = (typeof FINDING_LIFECYCLE_STATUSES)[number];

const LEGAL_TRANSITIONS: Record<FindingLifecycleStatus, readonly FindingLifecycleStatus[]> = {
  open: ["acknowledged", "resolved", "dismissed"],
  acknowledged: ["resolved", "dismissed"],
  resolved: ["reopened"],
  dismissed: ["reopened"],
  reopened: ["acknowledged", "resolved", "dismissed"],
};

export function canTransitionFinding(
  from: FindingLifecycleStatus,
  to: FindingLifecycleStatus,
): boolean {
  return LEGAL_TRANSITIONS[from].includes(to);
}

/** Entries in these statuses are candidates for re-matching against a new review's findings. */
export function isActiveFindingStatus(status: FindingLifecycleStatus): boolean {
  return status === "open" || status === "acknowledged" || status === "reopened";
}
