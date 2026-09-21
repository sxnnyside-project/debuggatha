import type { LineRange } from "./location.js";

/**
 * Structured, not a prompt — "future providers may generate different
 * textual representations from the same recommendation object" (see the
 * epic). Rendering a Recommendation into prose is a future concern
 * outside this engine.
 */
export const RECOMMENDATION_ACTIONS = [
  "replace",
  "remove",
  "add",
  "refactor",
  "configure",
  "document",
  "investigate",
] as const;
export type RecommendationAction = (typeof RECOMMENDATION_ACTIONS)[number];

export interface Recommendation {
  id: string;
  action: RecommendationAction;
  summary: string;
  rationale: string | undefined;
  targetFile: string | undefined;
  targetLines: LineRange | undefined;
  /** The same code before and after the fix, when the rule can show one. */
  example?: RecommendationExample;
  /** An exact text replacement, when the fix is mechanical enough to state precisely. */
  edit?: RecommendationEdit;
}

export interface RecommendationExample {
  before: string;
  after: string;
}

/**
 * Replace the text between two columns of one line. `safe` means the
 * replacement cannot change what the code does (`var` to `let`); `review`
 * means it is the right direction but a person or agent should check the
 * result still compiles and behaves (`any` to `unknown`).
 */
export interface RecommendationEdit {
  line: number;
  /** 1-based, inclusive. */
  startColumn: number;
  /** 1-based, just past the last replaced character. */
  endColumn: number;
  replacement: string;
  safety: "safe" | "review";
}
