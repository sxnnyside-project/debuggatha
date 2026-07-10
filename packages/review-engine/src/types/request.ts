import { randomId } from "../internal/ids.js";

/** Depth is defined here, not imported from `@debuggatha/shared` — see the package README's "Technical decisions". */
export const REVIEW_DEPTHS = ["quick", "full", "architectural"] as const;
export type ReviewDepth = (typeof REVIEW_DEPTHS)[number];

/**
 * One unified scope model instead of three separate request types — a
 * Diff Review, a File Review, and a Workspace Review differ only in
 * *what* is being reviewed, not in what a review request fundamentally
 * is.
 */
export type ReviewScope =
  | { kind: "diff"; base: string | undefined }
  | { kind: "files"; paths: string[] }
  | { kind: "workspace" };

export interface ReviewRequest {
  id: string;
  scope: ReviewScope;
  depth: ReviewDepth;
  requestedAt: string;
  requestedPackIds: string[];
  requestedPolicyId: string | undefined;
}

export interface CreateReviewRequestInput {
  id?: string;
  scope: ReviewScope;
  depth: ReviewDepth;
  requestedAt?: string;
  requestedPackIds?: string[];
  requestedPolicyId?: string;
}

export function createReviewRequest(input: CreateReviewRequestInput): ReviewRequest {
  if (input.scope.kind === "files" && input.scope.paths.length === 0) {
    throw new Error("A file review request must name at least one file.");
  }

  return {
    id: input.id ?? randomId(),
    scope: input.scope,
    depth: input.depth,
    requestedAt: input.requestedAt ?? new Date().toISOString(),
    requestedPackIds: input.requestedPackIds ?? [],
    requestedPolicyId: input.requestedPolicyId,
  };
}
