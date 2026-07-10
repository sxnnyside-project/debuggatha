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
}
