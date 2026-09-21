import { type Finding, ruleIdFromEvidence } from "@debuggatha/core";
import { ADAPTERS } from "./adapters.js";
import { type AnalyzerFindings, SourceLines, toFinding } from "./run.js";

export { ADAPTERS } from "./adapters.js";
export type { AnalyzerInventoryEntry, AnalyzerRunResult } from "./run.js";
export { findExecutable, inventoryAnalyzers, runAnalyzers } from "./run.js";
export { parseSarif } from "./sarif.js";
export type {
  AnalyzerAdapter,
  AnalyzerFinding,
  AnalyzerOptions,
  AnalyzerRun,
  AnalyzerStatus,
  AnalyzerTrust,
} from "./types.js";

/**
 * Adds what the analyzers found to what the built-in detectors found, without
 * saying the same thing twice. Where a tool reports the same rule at the same
 * line, its finding wins: it is the repository's own linter, configured by the
 * repository. A secret scanner answers for secrets entirely, so the built-in
 * secret heuristics step aside when one ran.
 */
export function mergeAnalyzerFindings(
  internal: readonly Finding[],
  results: readonly AnalyzerFindings[],
  rootDir: string,
  policyId: string,
): Finding[] {
  const superseded = new Set(results.flatMap(({ adapter }) => adapter.supersedes ?? []));
  const covered = new Set<string>();
  for (const { adapter, findings } of results) {
    for (const found of findings) {
      const equivalent = adapter.equivalentTo?.[found.ruleId];
      if (equivalent && found.line !== undefined) {
        covered.add(`${found.file}\0${found.line}\0${equivalent}`);
      }
    }
  }

  const kept = internal.filter((finding) => {
    const ruleId = ruleIdFromEvidence(finding.evidence);
    if (ruleId === undefined) return true;
    if (superseded.has(ruleId)) return false;
    const location = finding.locations[0];
    return !covered.has(`${location?.file}\0${location?.lines?.start}\0${ruleId}`);
  });

  const lines = new SourceLines(rootDir);
  const external = results.flatMap(({ adapter, version, findings }) =>
    findings.map((found) => toFinding(found, { adapter, version }, lines, policyId)),
  );
  return [...kept, ...external];
}

/** Ids of the analyzers a repository could have, so ledger entries of one that did not run are recognizable. */
export const ANALYZER_IDS: ReadonlySet<string> = new Set(ADAPTERS.map((adapter) => adapter.id));
