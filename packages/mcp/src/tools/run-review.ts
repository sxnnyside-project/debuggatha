import {
  type AnalyzerOptions,
  type AnalyzerRun,
  createLocalProvider,
  executeReview,
  executeReviewWithSemantics,
  type InlineSuppression,
  type ReviewChanges,
  type ReviewResult,
  type SemanticProvider,
  type SemanticRun,
  type SuppressedFinding,
  type SyncReport,
} from "@debuggatha/engine";
import type { DomainDeps } from "../domain-deps.js";
import type { ToolRuntimeContext } from "./shared.js";

/**
 * Thin adapter over `@debuggatha/core`'s `executeReview` — the shared
 * pipeline every review-shaped tool needs. This file used to
 * hand-roll that whole sequence itself; it now only translates this
 * package's own `RunReviewInput`/`DomainDeps` shapes into
 * `executeReview`'s, so the actual orchestration lives in exactly one
 * place instead of being duplicated between `@sxnnyside/debuggatha-mcp` and
 * `@sxnnyside/debuggatha-cli`.
 */

export type RunReviewScope =
  | { kind: "diff"; base: string | undefined; diff: string }
  | { kind: "files"; paths: string[] }
  | { kind: "workspace" };

export interface RunReviewInput {
  rootDir: string;
  scope: RunReviewScope;
  packIds: string[];
  policyId: string | undefined;
  persist: boolean;
  includeBaselined: boolean;
  /** `false` runs only the built-in detectors for this call. It can only narrow what the server allows. */
  analyzers?: boolean | undefined;
  /** Also have the server's configured model read the changed code. */
  semantic?: boolean | undefined;
}

export interface RunReviewOutput {
  analyzers: AnalyzerRun[];
  /** What the semantic pass did; `undefined` when it was not asked for. */
  semantic?: SemanticRun | undefined;
  result: ReviewResult;
  syncReport: SyncReport;
  suppressedFindings: SuppressedFinding[];
  suppressedInline: InlineSuppression[];
  baselined: number;
  ledgerIds: Record<string, string>;
  changes: ReviewChanges;
}

/** `DomainDeps` is a structural superset of `ReviewPipelineDeps` — passed straight through so mocked-domain tests keep working unchanged. */
export function pipelineDeps(deps: DomainDeps) {
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

/** What the server allows, narrowed by a call that asked for none. */
export function analyzersFor(
  ctx: Pick<ToolRuntimeContext, "config">,
  wanted: boolean | undefined,
): AnalyzerOptions {
  if (wanted === false) return { mode: "off" };
  return {
    ...ctx.config.analyzers,
    enableHint: (id) => `the server has to be started with DEBUGGATHA_ANALYZERS=${id}`,
  };
}

/** The model the operator configured for the semantic pass, or a message saying how to get one. */
async function semanticProvider(ctx: ToolRuntimeContext): Promise<SemanticProvider> {
  const { provider, model, url } = ctx.config.semantic;
  if (provider === "off") {
    throw new Error(
      "The semantic pass is not enabled on this server. Start it with DEBUGGATHA_SEMANTIC=sampling (the client's own model), ollama, or lmstudio.",
    );
  }
  if (provider === "sampling") {
    const host = ctx.sampling();
    if (!host) {
      throw new Error(
        "DEBUGGATHA_SEMANTIC=sampling, but the connected client does not support sampling. Use a client that does, or configure ollama or lmstudio.",
      );
    }
    return host;
  }
  return createLocalProvider(provider, {
    ...(model ? { model } : {}),
    ...(url ? { baseUrl: url } : {}),
  });
}

export async function runReview(
  ctx: ToolRuntimeContext,
  input: RunReviewInput,
): Promise<RunReviewOutput> {
  const pipelineInput = {
    rootDir: input.rootDir,
    scope: input.scope,
    // Review depth has no effect on the engine yet, so it is not offered to callers.
    depth: "full" as const,
    packIds: input.packIds,
    policyId: input.policyId,
    sourceName: "debuggatha-mcp",
    persist: input.persist,
    includeBaselined: input.includeBaselined,
    analyzers: analyzersFor(ctx, input.analyzers),
    ...(ctx.cache ? { cache: ctx.cache } : {}),
  };
  if (input.semantic !== true) return executeReview(pipelineInput, pipelineDeps(ctx.deps));
  return executeReviewWithSemantics(
    pipelineInput,
    { provider: await semanticProvider(ctx), verify: ctx.config.semantic.verify },
    pipelineDeps(ctx.deps),
  );
}
