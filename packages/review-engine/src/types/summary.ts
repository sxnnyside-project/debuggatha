import type { Finding } from "./finding.js";
import type { ReviewPackReference, ReviewPolicyReference } from "./reference.js";
import { SEVERITIES, type Severity } from "./severity.js";

/** Reusable by CLI, MCP, and VS Code alike — computed once by the engine, not per consumer. */
export interface ReviewSummary {
  totalFindings: number;
  findingsBySeverity: Record<Severity, number>;
  findingsByCategory: Record<string, number>;
  durationMs: number | undefined;
  appliedPacks: ReviewPackReference[];
  appliedPolicies: ReviewPolicyReference[];
}

export interface SummarizeFindingsOptions {
  durationMs?: number | undefined;
  appliedPacks?: ReviewPackReference[];
  appliedPolicies?: ReviewPolicyReference[];
}

export function summarizeFindings(
  findings: Finding[],
  options: SummarizeFindingsOptions = {},
): ReviewSummary {
  const findingsBySeverity = Object.fromEntries(
    SEVERITIES.map((severity) => [severity, 0]),
  ) as Record<Severity, number>;
  const findingsByCategory: Record<string, number> = {};

  for (const finding of findings) {
    findingsBySeverity[finding.severity] += 1;
    findingsByCategory[finding.category] = (findingsByCategory[finding.category] ?? 0) + 1;
  }

  return {
    totalFindings: findings.length,
    findingsBySeverity,
    findingsByCategory,
    durationMs: options.durationMs,
    appliedPacks: options.appliedPacks ?? [],
    appliedPolicies: options.appliedPolicies ?? [],
  };
}
