import { realpathSync } from "node:fs";
import { isAbsolute, join, relative, sep } from "node:path";
import {
  ANALYZER_IDS,
  type AnalyzerOptions,
  type AnalyzerRun,
  mergeAnalyzerFindings,
  runAnalyzers,
} from "./analyzers/index.js";
import {
  applyBaseline,
  type Baseline,
  createBaselineFrom,
  loadBaseline,
  saveBaseline,
} from "./baseline.js";
import type {
  BuildRepositoryContextOptions,
  Finding,
  RepositoryContextCache,
  ReviewDepth,
  ReviewPolicy,
  ReviewResult,
  Severity,
  SyncReport,
} from "./index.js";
import * as core from "./index.js";
import { parseUnifiedDiff } from "./skills/engine/diff-parser.js";
import type { InlineSuppression } from "./skills/engine/suppress.js";

/**
 * The one orchestration pipeline every review-shaped entry point runs —
 * Architecture Review, Diff Review, and File Review differ only in
 * `scope`, never in the steps below ("the review scope changes,
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
  /** Write the synchronized ledger back to disk. Defaults to `true`; `false` runs the review read-only. */
  persist?: boolean;
  /** Report findings the repository's baseline already accepted. Defaults to `false`: with a baseline, only new findings are reported. */
  includeBaselined?: boolean;
  /**
   * External analyzers to run beside the built-in detectors. Omitted, none run:
   * a library call stays deterministic. The CLI and the MCP server opt in.
   */
  analyzers?: AnalyzerOptions;
}

/** A ledger entry, described enough to talk about without fetching it. */
export interface FindingRef {
  ledgerId: string;
  title: string;
  file: string;
  line: number | undefined;
  ruleId: string | undefined;
  severity: Severity;
}

/**
 * The delta an agent needs after acting on a review: what this review found
 * for the first time, what it no longer finds because the code was fixed or the
 * file removed, and what came back after having been closed. `introduced` and
 * `reopened` leave out what the baseline hid.
 */
export interface ReviewChanges {
  introduced: FindingRef[];
  fixed: FindingRef[];
  reopened: FindingRef[];
}

export interface ReviewPipelineOutput {
  /** What the review reports: everything found, minus what memory, an inline comment, or the baseline set aside. */
  result: ReviewResult;
  syncReport: SyncReport;
  /** Findings an inline `debuggatha-ignore` comment hid, with the reason each gave. */
  suppressedInline: InlineSuppression[];
  /** How many findings the baseline hid; `0` when there is no baseline. */
  baselined: number;
  /** Each finding's ledger entry id (finding id to entry id): the identity that survives between reviews. Valid on disk only when the review persisted. */
  ledgerIds: Record<string, string>;
  /** What this review changed in the ledger's picture of the repository. */
  changes: ReviewChanges;
  /** Every external analyzer considered: which ran, which did not and why. Empty when analyzers were not asked for. */
  analyzers: AnalyzerRun[];
  /** Findings Repository Memory suppressed before they ever reached the ledger — surfaced, never hidden, per "suppression should remain explicit and traceable." Empty when the repository has no `.debuggatha/memory.json` yet. */
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

/**
 * Pack ids a review requests: the caller's explicit picks first (they win
 * rule conflicts, per `resolvePolicy`'s request-order precedence), then every
 * other registered pack. Rule Resolution already drops each rule whose scope
 * doesn't match the repository's detected capabilities, so requesting the
 * whole catalog is what makes a review stack-aware without the caller
 * naming packs.
 */
export function resolveRequestedPackIds(explicit: readonly string[]): string[] {
  const registered = core.defaultCapabilityRegistry.listPacks().map((pack) => pack.id);
  return [...new Set([...explicit, ...registered])];
}

export const defaultPipelineDeps: ReviewPipelineDeps = {
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

/**
 * A finding's identity in the ledger must not depend on how its file was
 * named, so every location is stored relative to the repository root, with
 * POSIX separators. A file review is handed absolute paths and a workspace
 * review walks relative ones; without this the same finding would be tracked
 * twice. Files outside the root stay absolute.
 */
export function toRepoRelative(rootDir: string, file: string): string {
  if (!isAbsolute(file)) return file.split(sep).join("/");
  const real = (path: string) => {
    try {
      return realpathSync.native(path);
    } catch {
      return path;
    }
  };
  const relativePath = relative(real(rootDir), real(file));
  if (relativePath === "" || relativePath.startsWith("..") || isAbsolute(relativePath)) return file;
  return relativePath.split(sep).join("/");
}

function relativizeFinding(finding: Finding, rootDir: string): Finding {
  const original = finding.locations[0]?.file;
  const relativeFile = original === undefined ? undefined : toRepoRelative(rootDir, original);
  return {
    ...finding,
    locations: finding.locations.map((location) => ({
      ...location,
      file: toRepoRelative(rootDir, location.file),
    })),
    recommendations: finding.recommendations.map((recommendation) => ({
      ...recommendation,
      // The id embeds the file it was made for, so it moves to the relative path with it.
      id:
        original !== undefined && relativeFile !== undefined
          ? recommendation.id.replace(original, relativeFile)
          : recommendation.id,
      targetFile:
        recommendation.targetFile === undefined
          ? undefined
          : toRepoRelative(rootDir, recommendation.targetFile),
    })),
    evidence: finding.evidence.map((evidence) => {
      if ("file" in evidence) return { ...evidence, file: toRepoRelative(rootDir, evidence.file) };
      if (evidence.kind === "criteria") {
        return { ...evidence, source: toRepoRelative(rootDir, evidence.source) };
      }
      return evidence;
    }),
  };
}

function runSkillForScope(
  deps: ReviewPipelineDeps,
  rootDir: string,
  scope: ReviewPipelineScope,
  policy: ReviewPolicy,
  options: { onSuppressed: (suppression: InlineSuppression) => void },
): Finding[] {
  switch (scope.kind) {
    case "diff":
      return deps.reviewDiff(scope.diff, policy, options);
    case "workspace":
      return deps.reviewArchitecture(rootDir, policy, options);
    case "files":
      // A relative path is relative to the repository, not to wherever the process happens to run.
      return deps.reviewFiles(
        scope.paths.map((path) => (isAbsolute(path) ? path : join(rootDir, path))),
        policy,
        options,
      );
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
  rootDir: string,
): { kind: "workspace" } | { kind: "files"; files: string[] } {
  if (scope.kind === "workspace") return { kind: "workspace" };
  if (scope.kind === "files") {
    return { kind: "files", files: scope.paths.map((path) => toRepoRelative(rootDir, path)) };
  }
  return {
    kind: "files",
    files: [...new Set(findings.flatMap((finding) => finding.locations.map((l) => l.file)))],
  };
}

/** The files a review looks at, for analyzers that can be limited to them; `undefined` is the whole repository. */
function scopeFiles(scope: ReviewPipelineScope, rootDir: string): string[] | undefined {
  switch (scope.kind) {
    case "workspace":
      return undefined;
    case "files":
      return scope.paths.map((path) => toRepoRelative(rootDir, path));
    case "diff":
      return [...new Set(parseUnifiedDiff(scope.diff).map((unit) => unit.file))];
  }
}

/**
 * An analyzer that did not run says nothing about its earlier findings, so the
 * sync must not read their absence as "fixed". Entries of such a tool that the
 * sync auto-resolved go back to what they were.
 */
function keepUnanalyzedEntries(
  previous: core.Ledger,
  updated: core.Ledger,
  report: SyncReport,
  ranAnalyzers: ReadonlySet<string>,
): { ledger: core.Ledger; report: SyncReport } {
  const before = new Map(previous.entries.map((entry) => [entry.id, entry]));
  const restore = new Set(
    report.autoResolvedEntryIds.filter((id) => {
      const tool = before.get(id)?.fingerprint.ruleId?.split(":")[0];
      if (tool === "semantic") return true; // A model never closes a finding by not repeating it.
      return tool !== undefined && ANALYZER_IDS.has(tool) && !ranAnalyzers.has(tool);
    }),
  );
  if (restore.size === 0) return { ledger: updated, report };
  return {
    ledger: {
      ...updated,
      entries: updated.entries.map((entry) =>
        restore.has(entry.id) ? (before.get(entry.id) ?? entry) : entry,
      ),
    },
    report: {
      ...report,
      autoResolvedEntryIds: report.autoResolvedEntryIds.filter((id) => !restore.has(id)),
    },
  };
}

/** What the first half of a review produced: everything the second half needs, and the findings a model may still refine. */
export interface ReviewStage {
  repositoryContext: core.RepositoryContext;
  policy: ReviewPolicy;
  session: core.ReviewSession;
  findings: Finding[];
  suppressedInline: InlineSuppression[];
  analyzerRuns: AnalyzerRun[];
}

export function executeReview(
  input: ReviewPipelineInput,
  deps: ReviewPipelineDeps = defaultPipelineDeps,
): ReviewPipelineOutput {
  const stage = analyzeReview(input, deps);
  return concludeReview(input, deps, stage, stage.findings);
}

/** Context, policy, built-in detectors, and analyzers: everything that does not depend on the ledger. */
export function analyzeReview(
  input: ReviewPipelineInput,
  deps: ReviewPipelineDeps = defaultPipelineDeps,
): ReviewStage {
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
    requestedPackIds: resolveRequestedPackIds(input.packIds),
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
  const suppressedInline: InlineSuppression[] = [];
  try {
    findings = runSkillForScope(deps, repositoryContext.rootDir, input.scope, policy, {
      onSuppressed: (suppression) =>
        suppressedInline.push({
          ...suppression,
          file: toRepoRelative(repositoryContext.rootDir, suppression.file),
        }),
    }).map((finding) => relativizeFinding(finding, repositoryContext.rootDir));
  } catch (error) {
    deps.transitionSession(session, "failed", {
      completedAt: new Date().toISOString(),
      error: error instanceof Error ? error.message : String(error),
    });
    throw error;
  }

  let analyzerRuns: AnalyzerRun[] = [];
  if (input.analyzers && input.analyzers.mode !== "off") {
    const outcome = runAnalyzers({
      rootDir: repositoryContext.rootDir,
      files: scopeFiles(input.scope, repositoryContext.rootDir),
      options: input.analyzers,
    });
    analyzerRuns = outcome.runs;
    findings = mergeAnalyzerFindings(
      findings,
      outcome.results,
      repositoryContext.rootDir,
      policy.id,
    );
  }

  return { repositoryContext, policy, session, findings, suppressedInline, analyzerRuns };
}

/**
 * Memory, baseline, and ledger: turns the findings a review ended with into
 * what it reports and what it records. `findings` is passed apart from the
 * stage because a semantic pass may have refined or added to them.
 */
export function concludeReview(
  input: ReviewPipelineInput,
  deps: ReviewPipelineDeps,
  stage: ReviewStage,
  findings: Finding[],
): ReviewPipelineOutput {
  const { repositoryContext, suppressedInline, analyzerRuns } = stage;
  let { session } = stage;

  // Repository Memory consumed automatically — "no manual
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
  const trackedResult = deps.createReviewResult({ session, findings: keptFindings });

  // Adoption mode: what the baseline accepted is still tracked in the ledger
  // (so nothing is forgotten), but it is not reported, and cannot fail a build.
  const baseline = input.includeBaselined ? undefined : loadBaseline(input.rootDir);
  const { fresh, baselined } = baseline
    ? applyBaseline(keptFindings, baseline)
    : { fresh: keptFindings, baselined: [] };
  const result =
    baselined.length > 0 ? deps.createReviewResult({ session, findings: fresh }) : trackedResult;

  const ledger = deps.loadLedger(input.rootDir);
  const synced = deps.synchronizeReviewResult(
    ledger,
    trackedResult,
    repositoryContext,
    deriveSyncScope(input.scope, keptFindings, repositoryContext.rootDir),
  );
  const { ledger: updatedLedger, report } = keepUnanalyzedEntries(
    ledger,
    synced.ledger,
    synced.report,
    new Set(analyzerRuns.filter((run) => run.status === "ran").map((run) => run.id)),
  );
  if (input.persist !== false) deps.saveLedger(updatedLedger);

  // Stable identity: each finding's ledger entry, so a caller can refer to it in the next call.
  const ledgerIds: Record<string, string> = {};
  for (const finding of keptFindings) {
    const match = core.defaultFindingMatcher(finding, [...updatedLedger.entries]);
    if (match.kind === "matched") ledgerIds[finding.id] = match.entryId;
  }
  const reportedEntryIds = new Set(fresh.map((finding) => ledgerIds[finding.id]));
  const entriesById = new Map(updatedLedger.entries.map((entry) => [entry.id, entry]));
  const refOf = (entryId: string): FindingRef | undefined => {
    const entry = entriesById.get(entryId);
    if (!entry) return undefined;
    const location = entry.latestFinding.locations[0];
    return {
      ledgerId: entry.id,
      title: entry.latestFinding.title,
      file: entry.fingerprint.file,
      line: location?.lines?.start,
      ruleId: entry.fingerprint.ruleId,
      severity: entry.latestFinding.severity,
    };
  };
  const refs = (ids: string[], onlyReported: boolean) =>
    ids
      .filter((id) => !onlyReported || reportedEntryIds.has(id))
      .map(refOf)
      .filter((ref): ref is FindingRef => ref !== undefined);

  return {
    result,
    syncReport: report,
    suppressedFindings: suppressed,
    suppressedInline,
    baselined: baselined.length,
    analyzers: analyzerRuns,
    ledgerIds,
    changes: {
      introduced: refs(report.newEntryIds, true),
      fixed: refs(report.autoResolvedEntryIds, false),
      reopened: refs(report.reopenedEntryIds, true),
    },
  };
}

/**
 * Records every finding the repository has today as accepted, so later reviews
 * report only what is new. Reads the whole repository and ignores any baseline
 * already there; writes only `.debuggatha/baseline.json`, never the ledger.
 * It has to run the same analyzers the reviews will, or every finding of one
 * they add later would look new.
 */
export function createBaseline(
  input: Pick<ReviewPipelineInput, "rootDir" | "sourceName" | "analyzers"> & {
    cache?: RepositoryContextCache;
    /** Extra packs, the same a review of this repository asks for. */
    packIds?: string[];
  },
  deps: ReviewPipelineDeps = defaultPipelineDeps,
): { baseline: Baseline; count: number; analyzers: AnalyzerRun[] } {
  const { result, analyzers } = executeReview(
    {
      rootDir: input.rootDir,
      scope: { kind: "workspace" },
      depth: "full",
      packIds: input.packIds ?? [],
      policyId: undefined,
      sourceName: input.sourceName,
      persist: false,
      includeBaselined: true,
      ...(input.analyzers ? { analyzers: input.analyzers } : {}),
      ...(input.cache ? { cache: input.cache } : {}),
    },
    deps,
  );
  const baseline = createBaselineFrom(result.findings);
  saveBaseline(input.rootDir, baseline);
  return { baseline, count: result.findings.length, analyzers };
}
