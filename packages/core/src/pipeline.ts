import type {
  BuildRepositoryContextOptions,
  Finding,
  RepositoryContextCache,
  ReviewDepth,
  ReviewPolicy,
  ReviewResult,
  SyncReport,
} from "./index.js";
import * as core from "./index.js";

/**
 * The one orchestration pipeline every review-shaped entry point runs —
 * Architecture Review, Diff Review, and File Review differ only in
 * `scope`, never in the steps below (Epic 11: "the review scope changes,
 * the review engine does not"). Before this, `@debuggatha/mcp`'s
 * `runReview` and `@debuggatha/cli`'s `review` command each hand-rolled
 * the same nine-step sequence (build context → assemble policy → create
 * request/session → run a Skill → transition the session → build a
 * result → sync the ledger); this function is that sequence, written
 * once, so both adapters (and any future one) call through it instead of
 * re-deriving it.
 */
export type ReviewPipelineScope =
  | { kind: "diff"; base: string | undefined; diff: string }
  | { kind: "files"; paths: string[] }
  | { kind: "workspace" };

export interface ReviewPipelineInput {
  rootDir: string;
  scope: ReviewPipelineScope;
  depth: ReviewDepth;
  packIds: string[];
  policyId: string | undefined;
  /** Identifies which adapter ran this review, for the session's execution metadata (e.g. "debuggatha-mcp", "debuggatha-cli"). */
  sourceName: string;
  cache?: RepositoryContextCache;
}

export interface ReviewPipelineOutput {
  result: ReviewResult;
  syncReport: SyncReport;
  /** Findings Repository Memory (Epic 15) suppressed before they ever reached the ledger — surfaced, never hidden, per "suppression should remain explicit and traceable." Empty when the repository has no `.debuggatha/memory.json` yet. */
  suppressedFindings: core.SuppressedFinding[];
}

/**
 * The subset of `@debuggatha/core`'s façade this pipeline calls.
 * Adapters that already mock the domain for their own tests (e.g.
 * `@debuggatha/mcp`'s `DomainDeps`) can pass their mock bundle straight
 * through here — it's a structural superset of this interface — instead
 * of maintaining a second orchestration path just to stay testable.
 * Defaults to the real `@debuggatha/core` functions.
 */
export interface ReviewPipelineDeps {
  buildRepositoryContext: typeof core.buildRepositoryContext;
  createReviewRequest: typeof core.createReviewRequest;
  assemblePolicy: typeof core.assemblePolicy;
  createReviewSession: typeof core.createReviewSession;
  transitionSession: typeof core.transitionSession;
  createReviewResult: typeof core.createReviewResult;
  reviewDiff: typeof core.reviewDiff;
  reviewArchitecture: typeof core.reviewArchitecture;
  reviewFiles: typeof core.reviewFiles;
  synchronizeReviewResult: typeof core.synchronizeReviewResult;
  loadLedger: typeof core.loadLedger;
  saveLedger: typeof core.saveLedger;
  loadMemoryStore: typeof core.loadMemoryStore;
  filterSuppressedFindings: typeof core.filterSuppressedFindings;
}

const defaultDeps: ReviewPipelineDeps = {
  buildRepositoryContext: core.buildRepositoryContext,
  createReviewRequest: core.createReviewRequest,
  assemblePolicy: core.assemblePolicy,
  createReviewSession: core.createReviewSession,
  transitionSession: core.transitionSession,
  createReviewResult: core.createReviewResult,
  reviewDiff: core.reviewDiff,
  reviewArchitecture: core.reviewArchitecture,
  reviewFiles: core.reviewFiles,
  synchronizeReviewResult: core.synchronizeReviewResult,
  loadLedger: core.loadLedger,
  saveLedger: core.saveLedger,
  loadMemoryStore: core.loadMemoryStore,
  filterSuppressedFindings: core.filterSuppressedFindings,
};

function runSkillForScope(
  deps: ReviewPipelineDeps,
  rootDir: string,
  scope: ReviewPipelineScope,
  policy: ReviewPolicy,
): Finding[] {
  switch (scope.kind) {
    case "diff":
      return deps.reviewDiff(scope.diff, policy);
    case "workspace":
      return deps.reviewArchitecture(rootDir, policy);
    case "files":
      return deps.reviewFiles(scope.paths, policy);
  }
}

/**
 * A "diff" review's affected-file list isn't independently knowable
 * (this package has no git access of its own — see `RepositoryProvider`)
 * so its sync scope is derived from whatever files the produced findings
 * actually cite. This is the conservative choice: it can never
 * auto-resolve a ledger entry for a file this review didn't look at.
 */
function deriveSyncScope(
  scope: ReviewPipelineScope,
  findings: Finding[],
): { kind: "workspace" } | { kind: "files"; files: string[] } {
  if (scope.kind === "workspace") return { kind: "workspace" };
  if (scope.kind === "files") return { kind: "files", files: scope.paths };
  return {
    kind: "files",
    files: [...new Set(findings.flatMap((finding) => finding.locations.map((l) => l.file)))],
  };
}

export function executeReview(
  input: ReviewPipelineInput,
  deps: ReviewPipelineDeps = defaultDeps,
): ReviewPipelineOutput {
  const repositoryContextOptions: BuildRepositoryContextOptions = {};
  if (input.cache) {
    repositoryContextOptions.cache = input.cache;
  }
  const repositoryContext = deps.buildRepositoryContext(input.rootDir, repositoryContextOptions);

  const requestScope =
    input.scope.kind === "diff"
      ? { kind: "diff" as const, base: input.scope.base }
      : input.scope.kind === "files"
        ? { kind: "files" as const, paths: input.scope.paths }
        : { kind: "workspace" as const };

  const request = deps.createReviewRequest({
    scope: requestScope,
    depth: input.depth,
    requestedPackIds: input.packIds,
    ...(input.policyId !== undefined ? { requestedPolicyId: input.policyId } : {}),
  });

  const policy = deps.assemblePolicy(repositoryContext, request);

  let session = deps.createReviewSession({
    request,
    repositoryContext,
    selectedPolicy: { id: policy.id },
    selectedPacks: policy.packRefs,
  });
  session = deps.transitionSession(session, "prepared");
  session = deps.transitionSession(session, "running", {
    source: { kind: "deterministic-analyzer", name: input.sourceName },
    startedAt: new Date().toISOString(),
  });

  let findings: Finding[];
  try {
    findings = runSkillForScope(deps, repositoryContext.rootDir, input.scope, policy);
  } catch (error) {
    deps.transitionSession(session, "failed", {
      completedAt: new Date().toISOString(),
      error: error instanceof Error ? error.message : String(error),
    });
    throw error;
  }

  // Repository Memory (Epic 15) consumed automatically — "no manual
  // intervention should be required." Only `active` memory ever
  // suppresses anything (see `filterSuppressedFindings`), so a
  // repository with no memory yet, or with only unconfirmed
  // detected/suggested items, behaves exactly as before this epic.
  const memoryStore = deps.loadMemoryStore(input.rootDir);
  const { findings: keptFindings, suppressed } = deps.filterSuppressedFindings(
    findings,
    memoryStore,
  );

  session = deps.transitionSession(session, "completed", { completedAt: new Date().toISOString() });
  const result = deps.createReviewResult({ session, findings: keptFindings });

  const ledger = deps.loadLedger(input.rootDir);
  const { ledger: updatedLedger, report } = deps.synchronizeReviewResult(
    ledger,
    result,
    repositoryContext,
    deriveSyncScope(input.scope, keptFindings),
  );
  deps.saveLedger(updatedLedger);

  return { result, syncReport: report, suppressedFindings: suppressed };
}
