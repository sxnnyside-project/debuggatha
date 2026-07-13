import { randomId } from "../internal/ids.js";
import type { MemoryHistoryEvent, MemoryOrigin } from "./history.js";
import { canTransitionMemory, type MemoryLifecycleStatus } from "./lifecycle.js";

/**
 * Never conflated with `@debuggatha/context-intelligence`'s
 * `ContextConfidence` (`documented`/`detected`/`inferred` — Epic 14):
 * this is Epic 15's own three levels, because memory is about
 * engineering *decisions*, not just extracted facts. `"user-confirmed"`
 * is the strongest — a human explicitly said "yes, remember this" (via
 * `confirmMemory`, the only path that can ever produce it). `"inferred"`
 * is never promoted to `"user-confirmed"` automatically; only an actual
 * user action (lifecycle transition to `"confirmed"`) does that.
 */
export type MemoryConfidence = "inferred" | "documented" | "user-confirmed";

export type MemoryCategory =
  | "accepted-deviation"
  | "suppression"
  | "exception"
  | "convention"
  | "review-history"
  | "decision";

export interface MemoryEvidence {
  file: string;
  detail: string;
}

interface MemoryItemBase {
  id: string;
  status: MemoryLifecycleStatus;
  confidence: MemoryConfidence;
  history: MemoryHistoryEvent[];
  createdAt: string;
  updatedAt: string;
  /** Required, never optional — "users should always understand why Debuggatha remembers something" (Epic 15 "User Experience"). */
  rationale: string;
  evidence: MemoryEvidence[];
}

/** A ruleId is required — an accepted deviation always names what it deviates from. `scopeFile: undefined` means repository-wide. */
export interface AcceptedDeviationItem extends MemoryItemBase {
  category: "accepted-deviation";
  ruleId: string;
  scopeFile: string | undefined;
}

/**
 * A recurring false positive a repository wants suppressed. `fingerprint`
 * intentionally mirrors `@debuggatha/findings-ledger`'s
 * `FindingFingerprint` shape (file + ruleId/category + optional content
 * anchor) — reused for matching (`matching.ts`), never redefined, so
 * suppression matching stays consistent with how the Ledger itself
 * recognizes "the same finding."
 */
export interface SuppressionItem extends MemoryItemBase {
  category: "suppression";
  fingerprintFile: string;
  fingerprintRuleId: string | undefined;
  fingerprintCategory: string;
}

export type ExceptionKind =
  | "layer-crossing"
  | "compatibility-workaround"
  | "legacy-integration"
  | "other";

export interface ExceptionItem extends MemoryItemBase {
  category: "exception";
  exceptionKind: ExceptionKind;
  /** The rule this exception applies to — undefined means "every rule" within `scopeFile` (e.g. a whole legacy integration directory is exempted). */
  ruleId: string | undefined;
  scopeFile: string | undefined;
}

/** Repository-specific convention — "complements Review Packs rather than replace them" (Epic 15): never a substitute for pack content, an addition to it. */
export interface ConventionItem extends MemoryItemBase {
  category: "convention";
  rule: string;
}

/**
 * Enriches the Findings Ledger — never duplicates it. `ledgerEntryId`
 * links back to a real `LedgerEntry.id` this item is annotating; this
 * package never stores its own copy of a finding's detection history
 * (first/last detected, resolved, reopened — that's the Ledger's job,
 * see `@debuggatha/findings-ledger`'s `HistoryEvent[]`).
 */
export interface ReviewHistoryNoteItem extends MemoryItemBase {
  category: "review-history";
  ledgerEntryId: string;
  note: string;
}

export interface DecisionItem extends MemoryItemBase {
  category: "decision";
  decision: string;
  documentationRef: string | undefined;
}

export type MemoryItem =
  | AcceptedDeviationItem
  | SuppressionItem
  | ExceptionItem
  | ConventionItem
  | ReviewHistoryNoteItem
  | DecisionItem;

export type CreateMemoryItemInput =
  | (Omit<AcceptedDeviationItem, "id" | "status" | "history" | "createdAt" | "updatedAt"> & {
      origin: MemoryOrigin;
    })
  | (Omit<SuppressionItem, "id" | "status" | "history" | "createdAt" | "updatedAt"> & {
      origin: MemoryOrigin;
    })
  | (Omit<ExceptionItem, "id" | "status" | "history" | "createdAt" | "updatedAt"> & {
      origin: MemoryOrigin;
    })
  | (Omit<ConventionItem, "id" | "status" | "history" | "createdAt" | "updatedAt"> & {
      origin: MemoryOrigin;
    })
  | (Omit<ReviewHistoryNoteItem, "id" | "status" | "history" | "createdAt" | "updatedAt"> & {
      origin: MemoryOrigin;
    })
  | (Omit<DecisionItem, "id" | "status" | "history" | "createdAt" | "updatedAt"> & {
      origin: MemoryOrigin;
    });

/** Every new memory item starts at `"detected"` — even a user-authored one, so the same lifecycle/confirmation path applies uniformly (see `api.ts#confirmMemory`). */
export function createMemoryItem(input: CreateMemoryItemInput): MemoryItem {
  const now = new Date().toISOString();
  const { origin, ...rest } = input;

  const event: MemoryHistoryEvent = {
    timestamp: now,
    previousState: undefined,
    newState: "detected",
    origin,
    comment: undefined,
  };

  return {
    ...rest,
    id: randomId(),
    status: "detected",
    history: [event],
    createdAt: now,
    updatedAt: now,
  } as MemoryItem;
}

/** Pure — returns a new item rather than mutating, same discipline as `transitionFinding` (Findings Ledger). */
export function transitionMemory(
  item: MemoryItem,
  to: MemoryLifecycleStatus,
  origin: MemoryOrigin,
  comment?: string,
): MemoryItem {
  if (!canTransitionMemory(item.status, to)) {
    throw new Error(`Illegal memory lifecycle transition: "${item.status}" -> "${to}".`);
  }

  const event: MemoryHistoryEvent = {
    timestamp: new Date().toISOString(),
    previousState: item.status,
    newState: to,
    origin,
    comment,
  };

  return {
    ...item,
    status: to,
    updatedAt: event.timestamp,
    history: [...item.history, event],
  };
}
