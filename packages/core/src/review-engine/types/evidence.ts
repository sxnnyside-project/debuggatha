import type { LineRange } from "./location.js";

/**
 * Machine-readable evidence — a discriminated union, not free text. This
 * mirrors the evidence-trail pattern `core/repository-intelligence`
 * already uses for stack/criteria facts, applied here to review findings
 * (see CLAUDE.md "never opine without evidence").
 */
export type Evidence =
  | { kind: "documentation"; file: string; excerpt: string }
  | { kind: "criteria"; ruleId: string; source: string }
  | { kind: "review-pack-rule"; packId: string; ruleId: string }
  | { kind: "framework-convention"; framework: string; detail: string }
  | { kind: "language-convention"; language: string; detail: string }
  | { kind: "code"; file: string; lines: LineRange | undefined; excerpt: string | undefined }
  | {
      /** A finding an external analyzer reported; Debuggatha ran the tool and read its output, it did not judge the code itself. */
      kind: "external-analyzer";
      tool: string;
      ruleId: string;
      version: string | undefined;
      /** The tool's own license, shown next to what it found. */
      license: string;
      url: string | undefined;
    }
  | {
      /**
       * A language model looked at this. As `verification` it judged a finding another
       * detector made; as `detection` it raised the finding itself, and `topic` names it.
       * A model's opinion is evidence to weigh, never a reason to close a finding.
       */
      kind: "semantic-review";
      role: "verification" | "detection";
      provider: string;
      model: string | undefined;
      /** Verification only: `confirmed` (looks real), `doubtful` (looks like a false positive), or `unsure`. */
      verdict: "confirmed" | "doubtful" | "unsure" | undefined;
      reason: string;
      topic: string | undefined;
    };

/**
 * The rule a finding traces back to. An external analyzer's rule is prefixed
 * with the tool (`ruff:F401`), so two tools' rules never collide in the ledger.
 */
export function ruleIdFromEvidence(evidence: readonly Evidence[]): string | undefined {
  for (const item of evidence) {
    if (item.kind === "review-pack-rule" || item.kind === "criteria") return item.ruleId;
    if (item.kind === "external-analyzer") return `${item.tool}:${item.ruleId}`;
    if (item.kind === "semantic-review" && item.role === "detection" && item.topic) {
      return `semantic:${item.topic}`;
    }
  }
  return undefined;
}
