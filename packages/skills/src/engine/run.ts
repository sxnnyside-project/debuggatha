import type { ReviewPolicy } from "@debuggatha/knowledge-system";
import { createFinding, type Evidence, type Finding } from "@debuggatha/review-engine";
import { detectorFor } from "./detectors.js";

/** One file (or diff hunk) handed to the engine, already read/parsed by the calling Skill. */
export interface SourceUnit {
  file: string;
  content: string;
  /** Maps each line of `content` to its real line number in the file — undefined means `content` is the whole file (line N of content is line N of the file). */
  lineNumbers: number[] | undefined;
}

/**
 * The one place that turns a resolved `ReviewPolicy` + a set of source
 * units into `Finding[]` — `reviewArchitecture`/`reviewDiff`/`reviewFiles`
 * differ only in *what* `SourceUnit[]` they hand this function, per the
 * epic's "the review scope changes, the review engine does not."
 *
 * Deterministic: same policy + same source units always produce the same
 * findings, in the same order (rules in policy order, files in input
 * order, lines top-to-bottom) — no model call, no randomness.
 */
export function runReviewSkillsEngine(policy: ReviewPolicy, units: SourceUnit[]): Finding[] {
  const findings: Finding[] = [];

  for (const rule of policy.rules) {
    const detector = detectorFor(rule.id);
    if (!detector) continue; // no detector registered — see engine/detectors.ts "Known limitations"

    for (const unit of units) {
      const hits = detector({
        file: unit.file,
        content: unit.content,
        lineNumbers: unit.lineNumbers,
      });

      for (const hit of hits) {
        const evidence: Evidence[] = [
          {
            kind: "code",
            file: unit.file,
            lines: hit.lines,
            excerpt: hit.excerpt,
          },
        ];
        if (rule.origin.kind === "pack") {
          evidence.push({ kind: "review-pack-rule", packId: rule.origin.packId, ruleId: rule.id });
        } else {
          evidence.push({ kind: "criteria", ruleId: rule.id, source: unit.file });
        }

        findings.push(
          createFinding({
            title: rule.statement.split(/[.\n]/)[0]?.trim() || rule.statement,
            explanation: `This finding exists because rule "${rule.id}" states: ${rule.statement}`,
            severity: rule.defaultSeverity ?? "medium",
            confidence: "medium",
            category: rule.category ?? "maintainability",
            locations: [{ file: unit.file, lines: hit.lines }],
            evidence,
            appliedPolicy: { id: policy.id },
            ...(rule.origin.kind === "pack"
              ? { originatingPack: { id: rule.origin.packId, version: rule.origin.packVersion } }
              : {}),
            recommendations: [
              {
                id: `${rule.id}-${unit.file}-${hit.lines?.start ?? 0}`,
                action: "refactor",
                summary: `Address "${rule.id}": ${rule.statement}`,
                rationale: rule.statement,
                targetFile: unit.file,
                targetLines: hit.lines,
              },
            ],
          }),
        );
      }
    }
  }

  return findings;
}
