import type { ExecutionMetadata } from "./execution.js";
import type { Finding } from "./finding.js";
import type { Recommendation } from "./recommendation.js";
import type { ReviewSession } from "./session.js";
import type { ReviewSummary } from "./summary.js";
import { summarizeFindings } from "./summary.js";

/**
 * The structured deliverable, separate from ReviewSession (the traceable
 * process record). Linked by `sessionId` rather than embedded in the
 * session, so consumers can list sessions without loading every result,
 * or fetch a result without the full session history.
 *
 * `findings[].recommendations` are fixes for a specific finding;
 * `recommendations` here are review-level (e.g. patterns spanning
 * multiple findings) — distinct concepts, not a duplication.
 */
export interface ReviewResult {
  sessionId: string;
  summary: ReviewSummary;
  findings: Finding[];
  recommendations: Recommendation[];
  execution: ExecutionMetadata;
}

export interface CreateReviewResultInput {
  session: ReviewSession;
  findings: Finding[];
  recommendations?: Recommendation[];
}

export function createReviewResult(input: CreateReviewResultInput): ReviewResult {
  if (input.session.status !== "completed") {
    throw new Error(
      `Cannot create a Review Result from a session with status "${input.session.status}" — only a "completed" session produces a result.`,
    );
  }

  return {
    sessionId: input.session.id,
    summary: summarizeFindings(input.findings, {
      durationMs: input.session.execution.durationMs,
      appliedPacks: input.session.selectedPacks,
      appliedPolicies: input.session.selectedPolicy ? [input.session.selectedPolicy] : [],
    }),
    findings: input.findings,
    recommendations: input.recommendations ?? [],
    execution: input.session.execution,
  };
}
