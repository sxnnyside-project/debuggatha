import type { ReviewPolicy } from "@debuggatha/core";
import { createFinding, type Evidence, type Finding } from "@debuggatha/core";
import { type DetectorInput, detectorFor } from "./detectors.js";
import { guidanceFor } from "./guidance.js";
import { lex } from "./lex.js";
import { findDirective, type InlineSuppression, parseDirectives } from "./suppress.js";

/** One file (or diff hunk) handed to the engine, already read/parsed by the calling Skill. */
export interface SourceUnit {
  file: string;
  content: string;
  /** Maps each line of `content` to its real line number in the file — undefined means `content` is the whole file (line N of content is line N of the file). */
  lineNumbers: number[] | undefined;
}

export interface RunOptions {
  /** Called for each hit an inline `debuggatha-ignore` comment hid, so suppression stays visible. */
  onSuppressed?: (suppression: InlineSuppression) => void;
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
export function runReviewSkillsEngine(
  policy: ReviewPolicy,
  units: SourceUnit[],
  options: RunOptions = {},
): Finding[] {
  const findings: Finding[] = [];

  const prepared = units.map((unit) => {
    const lexed = lex(unit.content, unit.file);
    const input: DetectorInput = {
      file: unit.file,
      content: unit.content,
      lineNumbers: unit.lineNumbers,
      masked: lexed.masked,
      strings: lexed.strings,
    };
    return { unit, input, directives: parseDirectives(lexed.comments) };
  });

  for (const rule of policy.rules) {
    const detector = detectorFor(rule.id);
    if (!detector) continue; // no detector registered — see engine/detectors.ts "Known limitations"
    const guidance = guidanceFor(rule.id);

    for (const { unit, input, directives } of prepared) {
      for (const hit of detector(input)) {
        const directive = findDirective(directives, hit.index + 1, rule.id);
        if (directive) {
          options.onSuppressed?.({
            file: unit.file,
            line: hit.lines?.start,
            ruleId: rule.id,
            reason: directive.reason,
          });
          continue;
        }

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

        const statement = rule.statement.split(/[.\n]/)[0]?.trim() || rule.statement;
        findings.push(
          createFinding({
            title: statement,
            explanation: guidance?.why ?? `Rule "${rule.id}": ${rule.statement}`,
            severity: hit.severity ?? rule.defaultSeverity ?? "medium",
            confidence: hit.confidence,
            category: rule.category ?? "maintainability",
            locations: [{ file: unit.file, lines: hit.lines, columns: hit.columns }],
            evidence,
            appliedPolicy: { id: policy.id },
            ...(rule.origin.kind === "pack"
              ? { originatingPack: { id: rule.origin.packId, version: rule.origin.packVersion } }
              : {}),
            recommendations: [
              {
                id: `${rule.id}-${unit.file}-${hit.lines?.start ?? 0}`,
                action: guidance?.action ?? "refactor",
                summary: guidance?.fix ?? `Address "${rule.id}": ${rule.statement}`,
                rationale: guidance?.why ?? rule.statement,
                targetFile: unit.file,
                targetLines: hit.lines,
                ...(guidance
                  ? { example: { before: guidance.before, after: guidance.after } }
                  : {}),
                ...(hit.edit && hit.lines ? { edit: { line: hit.lines.start, ...hit.edit } } : {}),
              },
            ],
          }),
        );
      }
    }
  }

  return findings;
}
