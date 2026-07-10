import type { Finding } from "@debuggatha/review-engine";
import { randomId } from "../internal/ids.js";
import type { ReviewAssociation } from "./association.js";
import type { FindingFingerprint } from "./fingerprint.js";
import type { HistoryEvent, TransitionOrigin } from "./history.js";
import { canTransitionFinding, type FindingLifecycleStatus } from "./lifecycle.js";

/**
 * One tracked issue across its entire lifetime. `id` is the *stable*
 * identity (epic §2) — distinct from `latestFinding.id`, which
 * `@debuggatha/review-engine` regenerates fresh every time a Skill
 * constructs a `Finding`. Many different `Finding.id` values, produced
 * across many review sessions, can all belong to the same `LedgerEntry.id`
 * when they represent the same underlying issue (see `matchFinding`).
 */
export interface LedgerEntry {
  id: string;
  fingerprint: FindingFingerprint;
  status: FindingLifecycleStatus;
  history: HistoryEvent[];
  latestFinding: Finding;
  review: ReviewAssociation;
}

export interface CreateLedgerEntryInput {
  fingerprint: FindingFingerprint;
  finding: Finding;
  review: ReviewAssociation;
  origin: TransitionOrigin;
}

export function createLedgerEntry(input: CreateLedgerEntryInput): LedgerEntry {
  const createdEvent: HistoryEvent = {
    timestamp: new Date().toISOString(),
    previousState: undefined,
    newState: "open",
    origin: input.origin,
    comment: undefined,
  };

  return {
    id: randomId(),
    fingerprint: input.fingerprint,
    status: "open",
    history: [createdEvent],
    latestFinding: input.finding,
    review: input.review,
  };
}

/**
 * Pure — returns a new entry rather than mutating, the same immutability
 * discipline `@debuggatha/review-engine`'s `transitionSession` already
 * established. History is append-only: this never edits or removes a
 * past `HistoryEvent`, only appends one.
 */
export function transitionFinding(
  entry: LedgerEntry,
  to: FindingLifecycleStatus,
  origin: TransitionOrigin,
  comment?: string,
): LedgerEntry {
  if (!canTransitionFinding(entry.status, to)) {
    throw new Error(`Illegal finding lifecycle transition: "${entry.status}" -> "${to}".`);
  }

  const event: HistoryEvent = {
    timestamp: new Date().toISOString(),
    previousState: entry.status,
    newState: to,
    origin,
    comment,
  };

  return {
    ...entry,
    status: to,
    history: [...entry.history, event],
  };
}

/** Refreshes what a still-detected finding looks like without changing its lifecycle status or history. */
export function refreshLedgerEntry(
  entry: LedgerEntry,
  finding: Finding,
  fingerprint: FindingFingerprint,
  review: ReviewAssociation,
): LedgerEntry {
  return {
    ...entry,
    fingerprint,
    latestFinding: finding,
    review: { ...entry.review, lastUpdatedBySessionId: review.lastUpdatedBySessionId },
  };
}
