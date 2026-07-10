/**
 * `SkillDescriptor` is the cheap, always-loadable index entry for a Skill —
 * "what exists" separate from "what it does when it runs" (Architecture
 * doc §2, the Claude Code Agent Skills / LSP capability-negotiation
 * lesson). `tier` is CLAUDE.md's Core/Review/Analysis taxonomy;
 * `ReviewSource` (review-engine) is the orthogonal axis of *what kind of
 * thing* produced a Finding — a Review Skill can be backed by any
 * `ReviewSource` (Architecture doc §3 domain-model table).
 */
export const SKILL_TIERS = ["core", "review", "analysis"] as const;
export type SkillTier = (typeof SKILL_TIERS)[number];

export interface SkillDescriptor {
  id: string;
  tier: SkillTier;
  description: string;
}
