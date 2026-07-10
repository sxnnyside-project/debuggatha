import type {
  BuildRepositoryContextOptions,
  Finding,
  ReviewDepth,
  ReviewPolicy,
  ReviewResult,
  SyncReport,
} from "@debuggatha/core";
import type { DomainDeps } from "../domain-deps.js";
import type { ToolRuntimeContext } from "./shared.js";

/**
 * The multi-step composition every review-shaped tool needs — this *is*
 * "orchestrate existing domain services" (epic "Architecture Principles"):
 * every step below calls a real, already-implemented domain function.
 * Nothing here computes a severity, resolves a rule, or judges code —
 * that's `reviewDiff`/`reviewArchitecture` (`@debuggatha/skills`), still
 * throw-stubs today (see CLAUDE.md), which is exactly why this always
 * currently ends in a clean, honest "not implemented" error rather than
 * a fabricated result.
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

function runSkillForScope(
  deps: DomainDeps,
  input: RunReviewInput,
  rootDir: string,
  policy: ReviewPolicy,
): Finding[] {
  switch (input.scope.kind) {
    case "diff":
      return deps.reviewDiff(input.scope.diff, policy);
    case "workspace":
      return deps.reviewArchitecture(rootDir, policy);
    case "files":
      return deps.reviewFiles(input.scope.paths, policy);
  }
}

/**
 * Never inferred from git — a "diff" review's affected-file list isn't
 * knowable without git access neither this package nor any domain
 * package has (see `@debuggatha/findings-ledger`'s own README on this
 * exact point). Scoping sync to only the files the produced findings
 * actually cite is the conservative choice: it can never auto-resolve a
 * finding outside what this review actually looked at.
 */
function deriveSyncScope(
  scope: RunReviewScope,
  findings: Finding[],
): { kind: "workspace" } | { kind: "files"; files: string[] } {
  if (scope.kind === "workspace") return { kind: "workspace" };
  if (scope.kind === "files") return { kind: "files", files: scope.paths };
  return {
    kind: "files",
    files: [...new Set(findings.flatMap((finding) => finding.locations.map((l) => l.file)))],
  };
}

export function runReview(ctx: ToolRuntimeContext, input: RunReviewInput): RunReviewOutput {
  const { deps } = ctx;

  const repositoryContextOptions: BuildRepositoryContextOptions = {};
  if (ctx.cache) {
    repositoryContextOptions.cache = ctx.cache;
  }
  const repositoryContext = deps.buildRepositoryContext(input.rootDir, repositoryContextOptions);

  const requestScope =
    input.scope.kind === "diff"
      ? { kind: "diff" as const, base: input.scope.base }
      : input.scope.kind === "files"
        ? { kind: "files" as const, paths: input.scope.paths }
        : { kind: "workspace" as const };

  const requestInput = {
    scope: requestScope,
    depth: input.depth,
    requestedPackIds: input.packIds,
    ...(input.policyId !== undefined ? { requestedPolicyId: input.policyId } : {}),
  };
  const request = deps.createReviewRequest(requestInput);

  const policy = deps.assemblePolicy(repositoryContext, request);

  let session = deps.createReviewSession({
    request,
    repositoryContext,
    selectedPolicy: { id: policy.id },
    selectedPacks: policy.packRefs,
  });
  session = deps.transitionSession(session, "prepared");
  session = deps.transitionSession(session, "running", {
    source: { kind: "deterministic-analyzer", name: "debuggatha-mcp" },
    startedAt: new Date().toISOString(),
  });

  let findings: Finding[];
  try {
    findings = runSkillForScope(deps, input, repositoryContext.rootDir, policy);
  } catch (error) {
    const failed = deps.transitionSession(session, "failed", {
      completedAt: new Date().toISOString(),
      error: error instanceof Error ? error.message : String(error),
    });
    ctx.logger.warn("review.failed", { sessionId: failed.id, scope: input.scope.kind });
    throw error;
  }

  session = deps.transitionSession(session, "completed", { completedAt: new Date().toISOString() });
  const result = deps.createReviewResult({ session, findings });

  const ledger = deps.loadLedger(input.rootDir);
  const { ledger: updatedLedger, report } = deps.synchronizeReviewResult(
    ledger,
    result,
    repositoryContext,
    deriveSyncScope(input.scope, findings),
  );
  deps.saveLedger(updatedLedger);

  return { result, syncReport: report };
}
