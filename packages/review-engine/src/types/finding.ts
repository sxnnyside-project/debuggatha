import { randomId } from "../internal/ids.js";
import type { Category } from "./category.js";
import type { Evidence } from "./evidence.js";
import type { FindingLocation } from "./location.js";
import type { Recommendation } from "./recommendation.js";
import type { ReviewPackReference, ReviewPolicyReference } from "./reference.js";
import type { Confidence, Severity } from "./severity.js";

/** A finding is a first-class domain object, never a bare string. */
export interface Finding {
  id: string;
  title: string;
  explanation: string;
  severity: Severity;
  confidence: Confidence;
  category: Category;
  locations: FindingLocation[];
  evidence: Evidence[];
  appliedPolicy: ReviewPolicyReference | undefined;
  originatingPack: ReviewPackReference | undefined;
  recommendations: Recommendation[];
}

/** Derived, not stored — a single source of truth instead of a hand-maintained parallel list. */
export function affectedFiles(finding: Finding): string[] {
  return [...new Set(finding.locations.map((location) => location.file))];
}

export interface CreateFindingInput {
  id?: string;
  title: string;
  explanation: string;
  severity: Severity;
  confidence: Confidence;
  category: Category;
  locations: FindingLocation[];
  evidence: Evidence[];
  appliedPolicy?: ReviewPolicyReference;
  originatingPack?: ReviewPackReference;
  recommendations?: Recommendation[];
}

/**
 * Enforces the two invariants a Finding cannot exist without: it must
 * point at something (`locations`), and it must be backed by evidence
 * (`evidence`) — see CLAUDE.md "never opine without evidence". This is
 * that principle as a runtime check, not just a comment.
 */
export function createFinding(input: CreateFindingInput): Finding {
  if (input.locations.length === 0) {
    throw new Error(
      `Finding "${input.title}" has no locations — a finding must point at something.`,
    );
  }
  if (input.evidence.length === 0) {
    throw new Error(
      `Finding "${input.title}" has no evidence. Debuggatha never opines without evidence — see CLAUDE.md.`,
    );
  }

  return {
    id: input.id ?? randomId(),
    title: input.title,
    explanation: input.explanation,
    severity: input.severity,
    confidence: input.confidence,
    category: input.category,
    locations: input.locations,
    evidence: input.evidence,
    appliedPolicy: input.appliedPolicy,
    originatingPack: input.originatingPack,
    recommendations: input.recommendations ?? [],
  };
}
