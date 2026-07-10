import type { RepositoryContext } from "@debuggatha/repository-intelligence";
import type {
  ReviewPackReference,
  ReviewPolicyReference,
  ReviewResult,
} from "@debuggatha/review-engine";
import { canonicalStringify, sha256Hex } from "../internal/hash.js";

/**
 * A lightweight reference to the repository state a finding was produced
 * against — not the whole `RepositoryContext` embedded. Persisting the
 * full snapshot on every entry would duplicate the same stack/dependency/
 * documentation data across every finding from the same review, bloating
 * the ledger file. `fingerprint` is a content hash, computed the same way
 * `@debuggatha/knowledge-system` hashes `RepositoryContext.criteria` for
 * `ReviewPolicy.id` — enough to recognize "the same repository state",
 * not enough to reconstruct it.
 */
export interface RepositorySnapshotRef {
  generatedAt: string;
  fingerprint: string;
}

export function snapshotRefFor(context: RepositoryContext): RepositorySnapshotRef {
  return {
    generatedAt: context.generatedAt,
    fingerprint: sha256Hex(
      canonicalStringify({
        stack: context.stack,
        dependencies: context.dependencies,
        documentation: context.documentation,
        criteria: context.criteria,
      }),
    ).slice(0, 16),
  };
}

/**
 * Complete traceability (epic §5): which review created the finding,
 * which review most recently confirmed it still exists, the repository
 * state it was produced against, and what knowledge produced it.
 */
export interface ReviewAssociation {
  createdBySessionId: string;
  lastUpdatedBySessionId: string;
  repositorySnapshot: RepositorySnapshotRef;
  appliedPolicy: ReviewPolicyReference | undefined;
  appliedPacks: ReviewPackReference[];
}

export function associationFor(
  result: ReviewResult,
  context: RepositoryContext,
): ReviewAssociation {
  return {
    createdBySessionId: result.sessionId,
    lastUpdatedBySessionId: result.sessionId,
    repositorySnapshot: snapshotRefFor(context),
    appliedPolicy: result.summary.appliedPolicies[0],
    appliedPacks: result.summary.appliedPacks,
  };
}
