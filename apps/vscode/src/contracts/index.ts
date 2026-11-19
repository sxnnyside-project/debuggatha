// The shapes the daemon sends, generated from the schemas of the Rust engine (see
// scripts/contracts.mjs). Import from here, never from the generated files.
export type {
  Confidence,
  Evidence,
  Finding,
  LedgerEntry,
  LifecycleStatus,
  Recommendation,
  Severity,
} from './ledger.js';
export type { RepositoryContext } from './repository-context.js';
export type {
  AnalyzerRun,
  Coverage,
  ReviewInputRequired,
  ReviewJobResult1 as ReviewJobResult,
  SemanticRun,
} from './review-job-result.js';

import type { LifecycleStatus, Severity } from './ledger.js';

/** The packs a Review Policy was assembled from; the rest of the policy is not shown. */
export interface ReviewPolicy {
  id: string;
  packRefs: { id: string; version: string }[];
}

export const SEVERITIES: readonly Severity[] = [
  'informational',
  'low',
  'medium',
  'high',
  'critical',
];

/** Findings still to act on: not resolved and not dismissed. */
export function isActiveFindingStatus(status: LifecycleStatus): boolean {
  return status === 'open' || status === 'acknowledged' || status === 'reopened';
}
