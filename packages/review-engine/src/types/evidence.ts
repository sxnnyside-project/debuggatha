import type { LineRange } from "./location.js";

/**
 * Machine-readable evidence — a discriminated union, not free text. This
 * mirrors the evidence-trail pattern `@debuggatha/repository-intelligence`
 * already uses for stack/criteria facts, applied here to review findings
 * (see CLAUDE.md "never opine without evidence").
 */
export type Evidence =
  | { kind: "documentation"; file: string; excerpt: string }
  | { kind: "criteria"; ruleId: string; source: string }
  | { kind: "review-pack-rule"; packId: string; ruleId: string }
  | { kind: "framework-convention"; framework: string; detail: string }
  | { kind: "language-convention"; language: string; detail: string }
  | { kind: "code"; file: string; lines: LineRange | undefined; excerpt: string | undefined };
