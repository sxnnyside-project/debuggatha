import type { ReviewPolicy } from "@debuggatha/knowledge-system";
import {
  createFinding,
  type Evidence,
  type Finding,
  type Severity,
} from "@debuggatha/review-engine";
import type {
  RuntimeEngine,
  RuntimeSelection,
  SemanticFindingCandidate,
} from "@debuggatha/runtime-engine";
import type { SourceUnit } from "./run.js";

const SEVERITY_DEFAULT: Severity = "medium";

/**
 * The Review Skills side of Epic 13's integration point: turns a Runtime
 * Engine's `SemanticFindingCandidate[]` into real `Finding[]` — the one
 * place in this package that knows what a `RuntimeEngine` is. No Review
 * Skill imports `@debuggatha/runtime-engine`'s `Provider`,
 * `createOllamaProvider`, or `createLMStudioProvider` — only this
 * package-level function, and only the provider-agnostic `RuntimeEngine`
 * facade (Epic 13 "No Review Skill should know which provider is being
 * used" / "Do not introduce provider-specific logic into Review Skills").
 *
 * This is deliberately a separate, opt-in function rather than a change
 * to `reviewFiles`/`reviewDiff`/`reviewArchitecture`'s signatures: those
 * three are synchronous today, and every adapter (`@debuggatha/core`'s
 * `executeReview`, MCP, CLI, VS Code) calls them synchronously. Making
 * semantic execution mandatory would mean making all three — and
 * everything that calls them — async, which is a much larger, riskier
 * change than this epic's scope justifies. A caller that wants semantic
 * findings composes them explicitly:
 *
 * ```ts
 * const deterministic = reviewFiles(paths, policy);
 * const semantic = await runSemanticFindings(runtime, units, policy);
 * const findings = [...deterministic, ...semantic];
 * ```
 *
 * — "Deterministic analysis → Semantic review → Final findings" (Epic 13
 * "Integration"), composed by the caller, not hidden inside a skill
 * function that used to be synchronous.
 */
export async function runSemanticFindings(
  runtime: RuntimeEngine,
  units: SourceUnit[],
  policy: ReviewPolicy,
  options: { selection?: RuntimeSelection; signal?: AbortSignal } = {},
): Promise<Finding[]> {
  const { result } = await runtime.runSemanticReview({
    units: units.map((unit) => ({ file: unit.file, content: unit.content })),
    selection: options.selection ?? { mode: "automatic" },
    policyStatements: policy.rules.map((rule) => rule.statement),
    ...(options.signal ? { signal: options.signal } : {}),
  });

  return result.candidates.map((candidate) => candidateToFinding(candidate, policy));
}

function candidateToFinding(candidate: SemanticFindingCandidate, policy: ReviewPolicy): Finding {
  const evidence: Evidence[] = candidate.evidence.map((e) => ({
    kind: "code",
    file: e.file,
    lines: e.lines,
    excerpt: e.excerpt,
  }));

  return createFinding({
    title: candidate.title,
    explanation: `Semantic review finding (model-assisted, confidence: ${candidate.confidence}): ${candidate.explanation}`,
    severity: candidate.severityHint ?? SEVERITY_DEFAULT,
    confidence: candidate.confidence,
    category: "maintainability",
    locations: candidate.evidence.map((e) => ({ file: e.file, lines: e.lines })),
    evidence,
    appliedPolicy: { id: policy.id },
    recommendations: candidate.recommendation
      ? [
          {
            id: `semantic-${candidate.title.toLowerCase().replace(/[^a-z0-9]+/g, "-")}`,
            action: "investigate",
            summary: candidate.recommendation,
            rationale: candidate.explanation,
            targetFile: candidate.evidence[0]?.file,
            targetLines: candidate.evidence[0]?.lines,
          },
        ]
      : [],
  });
}
