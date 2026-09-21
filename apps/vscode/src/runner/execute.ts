import {
  createLocalProvider,
  executeReview,
  executeReviewWithSemantics,
  type ReviewPipelineInput,
  type SemanticRun,
} from "@debuggatha/engine";
import type { ReviewJob, ReviewJobResult } from "./protocol.js";

/** Runs a job with the engine. The one place a review is executed, whichever process it is in. */
export async function executeJob(job: ReviewJob): Promise<ReviewJobResult> {
  const started = Date.now();
  const input: ReviewPipelineInput = {
    rootDir: job.rootDir,
    scope: job.scope,
    depth: "full",
    packIds: job.packIds,
    policyId: undefined,
    sourceName: "vscode",
    analyzers: job.analyzers,
  };

  const output: ReturnType<typeof executeReview> & { semantic?: SemanticRun } = job.semantic
    ? await executeReviewWithSemantics(input, {
        provider: await createLocalProvider(job.semantic.provider, {
          ...(job.semantic.model ? { model: job.semantic.model } : {}),
          ...(job.semantic.url ? { baseUrl: job.semantic.url } : {}),
        }),
        verify: false,
      })
    : executeReview(input);

  return {
    findings: output.result.findings.length,
    analyzers: output.analyzers,
    semantic: output.semantic ?? null,
    changes: {
      introduced: output.changes.introduced.length,
      fixed: output.changes.fixed.length,
      reopened: output.changes.reopened.length,
    },
    durationMs: Date.now() - started,
  };
}
