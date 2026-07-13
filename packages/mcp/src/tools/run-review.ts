import {
  executeReview,
  type ReviewDepth,
  type ReviewResult,
  type SyncReport,
} from "@debuggatha/core";
import type { DomainDeps } from "../domain-deps.js";
import type { ToolRuntimeContext } from "./shared.js";

/**
 * Thin adapter over `@debuggatha/core`'s `executeReview` — the shared
 * pipeline every review-shaped tool needs (Epic 11). This file used to
 * hand-roll that whole sequence itself; it now only translates this
 * package's own `RunReviewInput`/`DomainDeps` shapes into
 * `executeReview`'s, so the actual orchestration lives in exactly one
 * place instead of being duplicated between `@debuggatha/mcp` and
 * `@debuggatha/cli`.
 */

export type RunReviewScope =
  | { kind: "diff"; base: string | undefined; diff: string }
  | { kind: "files"; paths: string[] }
  | { kind: "workspace" };

export interface RunReviewInput {
  rootDir: string;
  scope: RunReviewScope;
  depth: ReviewDepth;
  packIds: string[];
  policyId: string | undefined;
}

export interface RunReviewOutput {
  result: ReviewResult;
  syncReport: SyncReport;
}

/** `DomainDeps` is a structural superset of `ReviewPipelineDeps` — passed straight through so mocked-domain tests keep working unchanged. */
function pipelineDeps(deps: DomainDeps) {
  return {
    buildRepositoryContext: deps.buildRepositoryContext,
    createReviewRequest: deps.createReviewRequest,
    assemblePolicy: deps.assemblePolicy,
    createReviewSession: deps.createReviewSession,
    transitionSession: deps.transitionSession,
    createReviewResult: deps.createReviewResult,
    reviewDiff: deps.reviewDiff,
    reviewArchitecture: deps.reviewArchitecture,
    reviewFiles: deps.reviewFiles,
    synchronizeReviewResult: deps.synchronizeReviewResult,
    loadLedger: deps.loadLedger,
    saveLedger: deps.saveLedger,
    loadMemoryStore: deps.loadMemoryStore,
    filterSuppressedFindings: deps.filterSuppressedFindings,
  };
}

export function runReview(ctx: ToolRuntimeContext, input: RunReviewInput): RunReviewOutput {
  return executeReview(
    {
      rootDir: input.rootDir,
      scope: input.scope,
      depth: input.depth,
      packIds: input.packIds,
      policyId: input.policyId,
      sourceName: "debuggatha-mcp",
      ...(ctx.cache ? { cache: ctx.cache } : {}),
    },
    pipelineDeps(ctx.deps),
  );
}
