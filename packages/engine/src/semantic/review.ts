import { readFileSync } from "node:fs";
import { join } from "node:path";
import { type Finding, ruleIdFromEvidence } from "@debuggatha/core";
import {
  analyzeReview,
  concludeReview,
  defaultPipelineDeps,
  type ReviewPipelineDeps,
  type ReviewPipelineInput,
  type ReviewPipelineOutput,
  type ReviewPipelineScope,
  toRepoRelative,
} from "../pipeline.js";
import { isIgnoredPath } from "../scan-scope/index.js";
import { parseUnifiedDiff } from "../skills/engine/diff-parser.js";
import {
  type CodeView,
  detectionPrompt,
  detectionsFrom,
  isSecretFile,
  judge,
  windowAround,
  withVerdict,
} from "./judge.js";
import { DEFAULT_SEMANTIC_LIMITS, type SemanticOptions, type SemanticRun } from "./types.js";

const CODE_FILE = /\.(m|c)?[jt]sx?$|\.(py|kts?|java|php|rs|go|rb|cs|swift|dart)$/;
const SEVERITY_ORDER = ["critical", "high", "medium", "low", "informational"];
/** Calls in a row that may fail before the model is given up on for this review. */
const MAX_CONSECUTIVE_FAILURES = 3;

const isSecretRule = (ruleId: string | undefined) =>
  ruleId === "no-hardcoded-secrets" || ruleId?.startsWith("gitleaks:") === true;

/** The changed code a model may read: what the review was asked about, and nothing else. */
function detectionViews(rootDir: string, scope: ReviewPipelineScope, maxLines: number): CodeView[] {
  if (scope.kind === "workspace") return [];
  if (scope.kind === "diff") {
    return parseUnifiedDiff(scope.diff).map((unit) => ({
      file: unit.file,
      lines: unit.content
        .split("\n")
        .slice(0, maxLines)
        .map((text, index) => ({ n: unit.lineNumbers?.[index] ?? index + 1, text })),
    }));
  }
  const views: CodeView[] = [];
  for (const path of scope.paths) {
    const file = toRepoRelative(rootDir, path);
    try {
      const text = readFileSync(path.startsWith("/") ? path : join(rootDir, path), "utf8");
      views.push({
        file,
        lines: text
          .split("\n")
          .slice(0, maxLines)
          .map((line, index) => ({ n: index + 1, text: line })),
      });
    } catch {
      // A file that is gone has nothing to read.
    }
  }
  return views;
}

/**
 * Asks a model about what a review found and what changed, and returns the findings with
 * its opinions attached. It never removes a finding; it fails soft, so a model that is down
 * or confused leaves the review exactly as the analyzers made it.
 */
export async function refineFindings(input: {
  rootDir: string;
  scope: ReviewPipelineScope;
  findings: Finding[];
  policyId: string;
  options: SemanticOptions;
}): Promise<{ findings: Finding[]; run: SemanticRun }> {
  const { rootDir, scope, options, policyId } = input;
  const { provider } = options;
  const started = Date.now();
  const limits = { ...DEFAULT_SEMANTIC_LIMITS, ...stripUndefined(options) };
  const run: SemanticRun = {
    provider: provider.name,
    model: provider.model,
    verified: 0,
    confirmed: 0,
    doubtful: 0,
    unsure: 0,
    detected: 0,
    rejected: 0,
    failed: 0,
    notes: [],
    durationMs: 0,
  };
  let consecutive = 0;
  const alive = () => consecutive < MAX_CONSECUTIVE_FAILURES;
  const failed = (error: unknown) => {
    run.failed += 1;
    consecutive += 1;
    if (run.notes.length < 3) {
      run.notes.push(error instanceof Error ? error.message : String(error));
    }
  };

  // Lines that may hold a secret are never shown to a model.
  const withheld = new Map<string, Set<number>>();
  for (const finding of input.findings) {
    if (!isSecretRule(ruleIdFromEvidence(finding.evidence))) continue;
    const location = finding.locations[0];
    if (!location?.lines) continue;
    const set = withheld.get(location.file) ?? new Set<number>();
    for (let n = location.lines.start; n <= location.lines.end; n++) set.add(n);
    withheld.set(location.file, set);
  }
  const hidden = (file: string) => withheld.get(file) ?? new Set<number>();

  let findings = input.findings;

  if (options.verify === true) {
    const candidates = findings
      .filter((finding) => {
        const ruleId = ruleIdFromEvidence(finding.evidence);
        const file = finding.locations[0]?.file ?? "";
        return (
          finding.confidence !== "high" &&
          !isSecretRule(ruleId) &&
          !isSecretFile(file) &&
          !finding.evidence.some(
            (evidence) =>
              evidence.kind === "external-analyzer" || evidence.kind === "semantic-review",
          ) &&
          finding.locations[0]?.lines !== undefined
        );
      })
      .sort((a, b) => SEVERITY_ORDER.indexOf(a.severity) - SEVERITY_ORDER.indexOf(b.severity))
      .slice(0, limits.maxCandidates);

    const verdicts = new Map<string, Finding>();
    for (const finding of candidates) {
      if (!alive()) break;
      const location = finding.locations[0];
      const line = location?.lines?.start;
      if (!location || line === undefined) continue;
      const view = windowAround(rootDir, location.file, line);
      if (!view) continue;
      try {
        const verification = await judge(provider, finding, view, hidden(location.file));
        if (!verification) {
          run.failed += 1;
          consecutive += 1;
          continue;
        }
        consecutive = 0;
        run.verified += 1;
        run[verification.verdict] += 1;
        verdicts.set(finding.id, withVerdict(finding, verification, provider));
      } catch (error) {
        failed(error);
      }
    }
    findings = findings.map((finding) => verdicts.get(finding.id) ?? finding);
  }

  if (options.detect !== false && scope.kind === "workspace") {
    run.notes.push(
      "Detection reads changed code; a whole-repository review has none, so only candidates were verified.",
    );
  }
  if (options.detect !== false && scope.kind !== "workspace") {
    const views = detectionViews(rootDir, scope, limits.maxLines)
      .filter(
        (view) =>
          CODE_FILE.test(view.file) &&
          !isSecretFile(view.file) &&
          !isIgnoredPath(rootDir, view.file),
      )
      .slice(0, limits.maxDetectFiles);
    const added: Finding[] = [];
    for (const view of views) {
      if (!alive()) break;
      try {
        const reply = await provider.complete(detectionPrompt(view, hidden(view.file)));
        consecutive = 0;
        const { accepted, rejected } = detectionsFrom(reply, view, provider, policyId);
        run.rejected += rejected;
        // Whatever a detector already said about this very line is not said again.
        for (const finding of accepted) {
          const at = finding.locations[0];
          const near = findings.some(
            (existing) =>
              existing.locations[0]?.file === at?.file &&
              existing.locations[0]?.lines?.start === at?.lines?.start,
          );
          if (near) continue;
          added.push(finding);
        }
      } catch (error) {
        failed(error);
      }
    }
    run.detected = added.length;
    findings = [...findings, ...added];
  }

  if (!alive())
    run.notes.push("The model failed repeatedly, so the rest of the semantic pass was skipped.");
  // A host reveals its model only in its first answer.
  run.model = provider.model;
  run.durationMs = Date.now() - started;
  return { findings, run };
}

function stripUndefined(options: SemanticOptions) {
  const limits: Partial<Record<"maxCandidates" | "maxDetectFiles" | "maxLines", number>> = {};
  for (const key of ["maxCandidates", "maxDetectFiles", "maxLines"] as const) {
    const value = options[key];
    if (value !== undefined) limits[key] = value;
  }
  return limits;
}

export interface SemanticReviewOutput extends ReviewPipelineOutput {
  semantic: SemanticRun;
}

/** `executeReview` with a model's opinion between the analyzers and the ledger. */
export async function executeReviewWithSemantics(
  input: ReviewPipelineInput,
  options: SemanticOptions,
  deps: ReviewPipelineDeps = defaultPipelineDeps,
): Promise<SemanticReviewOutput> {
  const stage = analyzeReview(input, deps);
  const { findings, run } = await refineFindings({
    rootDir: stage.repositoryContext.rootDir,
    scope: input.scope,
    findings: stage.findings,
    policyId: stage.policy.id,
    options,
  });
  return { ...concludeReview(input, deps, stage, findings), semantic: run };
}
