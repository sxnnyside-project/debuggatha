/**
 * Extensibility point (epic §11): the domain does not assume AI is
 * always involved. A finding or session records what produced it without
 * privileging any one kind.
 */
export const REVIEW_SOURCE_KINDS = [
  "ai-provider",
  "deterministic-analyzer",
  "custom-skill",
  "human-review",
] as const;
export type ReviewSourceKind = (typeof REVIEW_SOURCE_KINDS)[number];

export interface ReviewSource {
  kind: ReviewSourceKind;
  name: string;
}
