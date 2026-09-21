import type { RepositoryContext } from "../../repository-intelligence/index.js";
import { randomId } from "../internal/ids.js";
import { type ExecutionMetadata, initialExecutionMetadata } from "./execution.js";
import { canTransition, type ReviewLifecycleStatus } from "./lifecycle.js";
import type { ReviewPackReference, ReviewPolicyReference } from "./reference.js";
import type { ReviewRequest } from "./request.js";

/**
 * The traceable record of one review in progress. Holds the
 * RepositoryContext by reference (it is already an immutable, frozen
 * value — no copying, no separate lookup needed) plus which
 * policy/packs were selected and the lifecycle status.
 */
export interface ReviewSession {
  id: string;
  createdAt: string;
  request: ReviewRequest;
  repositoryContext: RepositoryContext;
  selectedPolicy: ReviewPolicyReference | undefined;
  selectedPacks: ReviewPackReference[];
  status: ReviewLifecycleStatus;
  execution: ExecutionMetadata;
}

export interface CreateReviewSessionInput {
  id?: string;
  createdAt?: string;
  request: ReviewRequest;
  repositoryContext: RepositoryContext;
  selectedPolicy?: ReviewPolicyReference;
  selectedPacks?: ReviewPackReference[];
}

export function createReviewSession(input: CreateReviewSessionInput): ReviewSession {
  return {
    id: input.id ?? randomId(),
    createdAt: input.createdAt ?? new Date().toISOString(),
    request: input.request,
    repositoryContext: input.repositoryContext,
    selectedPolicy: input.selectedPolicy,
    selectedPacks: input.selectedPacks ?? [],
    status: "requested",
    execution: initialExecutionMetadata(),
  };
}

/**
 * Pure — returns a new session rather than mutating, matching the
 * immutability discipline `RepositoryContext` already established.
 * Throws on any transition `canTransition` doesn't allow.
 */
export function transitionSession(
  session: ReviewSession,
  to: ReviewLifecycleStatus,
  patch: Partial<ExecutionMetadata> = {},
): ReviewSession {
  if (!canTransition(session.status, to)) {
    throw new Error(`Illegal review lifecycle transition: "${session.status}" -> "${to}".`);
  }

  return {
    ...session,
    status: to,
    execution: { ...session.execution, ...patch },
  };
}
