import type {
  SemanticConfidence,
  SemanticEvidence,
  SemanticFindingCandidate,
  SemanticReviewResult,
} from "./types.js";

const CONFIDENCE_VALUES: SemanticConfidence[] = ["high", "medium", "low"];
const SEVERITY_VALUES = ["critical", "high", "medium", "low", "informational"];

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function parseEvidence(raw: unknown): SemanticEvidence[] {
  if (!Array.isArray(raw)) return [];
  const evidence: SemanticEvidence[] = [];
  for (const entry of raw) {
    if (!isRecord(entry) || typeof entry.file !== "string" || typeof entry.excerpt !== "string")
      continue;
    let lines: SemanticEvidence["lines"];
    if (
      isRecord(entry.lines) &&
      typeof entry.lines.start === "number" &&
      typeof entry.lines.end === "number"
    ) {
      lines = { start: entry.lines.start, end: entry.lines.end };
    }
    evidence.push({ file: entry.file, lines, excerpt: entry.excerpt });
  }
  return evidence;
}

function parseCandidate(raw: unknown): SemanticFindingCandidate | undefined {
  if (!isRecord(raw)) return undefined;
  if (typeof raw.title !== "string" || typeof raw.explanation !== "string") return undefined;

  const evidence = parseEvidence(raw.evidence);
  // A candidate with no evidence is not a candidate — same "never opine
  // without evidence" invariant `createFinding` (review-engine)
  // enforces at construction time, applied here before this ever reaches
  // that boundary.
  if (evidence.length === 0) return undefined;

  const confidence: SemanticConfidence = CONFIDENCE_VALUES.includes(
    raw.confidence as SemanticConfidence,
  )
    ? (raw.confidence as SemanticConfidence)
    : "low";

  const severityHint = SEVERITY_VALUES.includes(raw.severityHint as string)
    ? (raw.severityHint as SemanticFindingCandidate["severityHint"])
    : undefined;

  return {
    title: raw.title,
    explanation: raw.explanation,
    confidence,
    severityHint,
    evidence,
    recommendation: typeof raw.recommendation === "string" ? raw.recommendation : undefined,
  };
}

/**
 * Turns a provider's raw text response into a `SemanticReviewResult` —
 * the boundary where free-form model output either becomes the
 * normalized candidate shape or is honestly reported as unparseable.
 * Never throws: a malformed response produces zero candidates plus a
 * `parseWarnings` entry, not a crash — a semantic review failing to
 * parse should degrade to "no semantic findings this run," not take
 * down the whole review (deterministic findings from the same run are
 * unaffected).
 *
 * Expects a JSON object (optionally fenced in a ```json code block, the
 * common way chat-tuned models wrap structured output) shaped
 * `{ candidates: [...] }`; each candidate needs at least `title`,
 * `explanation`, and non-empty `evidence` to survive — anything else is
 * dropped with a warning, never fabricated into a fuller shape.
 */
export function parseSemanticResponse(raw: string): SemanticReviewResult {
  const jsonText = extractJson(raw);
  if (jsonText === undefined) {
    return {
      candidates: [],
      parseWarnings: ["Response did not contain a recognizable JSON object."],
    };
  }

  let parsed: unknown;
  try {
    parsed = JSON.parse(jsonText);
  } catch (error) {
    return {
      candidates: [],
      parseWarnings: [
        `Response JSON failed to parse: ${error instanceof Error ? error.message : String(error)}`,
      ],
    };
  }

  if (!isRecord(parsed) || !Array.isArray(parsed.candidates)) {
    return { candidates: [], parseWarnings: ['Parsed JSON has no "candidates" array.'] };
  }

  const candidates: SemanticFindingCandidate[] = [];
  const parseWarnings: string[] = [];
  parsed.candidates.forEach((entry, index) => {
    const candidate = parseCandidate(entry);
    if (candidate) candidates.push(candidate);
    else
      parseWarnings.push(`candidates[${index}] was dropped — missing title/explanation/evidence.`);
  });

  return { candidates, parseWarnings };
}

function extractJson(raw: string): string | undefined {
  const fenced = /```(?:json)?\s*([\s\S]*?)```/.exec(raw);
  const candidate = (fenced?.[1] ?? raw).trim();
  const start = candidate.indexOf("{");
  const end = candidate.lastIndexOf("}");
  if (start === -1 || end === -1 || end < start) return undefined;
  return candidate.slice(start, end + 1);
}
