import type { ReviewPolicy } from "@debuggatha/knowledge-system";
import type { Finding } from "@debuggatha/review-engine";
import { buildAnalysisFindings } from "../engine/analysis-findings.js";
import { walkSourceUnits } from "../engine/read-files.js";
import { runReviewSkillsEngine } from "../engine/run.js";

/**
 * Review Skill: Architecture Review.
 *
 * Reviews the whole repository tree for structural/layering concerns:
 * the assembled policy's rules run against every source file (see
 * `runReviewSkillsEngine`), plus layer violations and dependency cycles
 * from `@debuggatha/analysis-engine`'s deterministic pipeline (Epic 12) —
 * covering "architectural boundaries, layering, dependency direction,
 * repository structure" from the epic without requiring per-framework
 * Review Pack content for the structural check specifically, and without
 * this Skill re-walking the dependency graph itself (Epic 12: "Review
 * Skills should consume analysis results instead of recomputing them").
 *
 * Detecting *which* architectural pattern is in use (Clean Architecture,
 * Hexagonal, MVC, MVVM, Feature-First, Layered) and adapting critique to
 * it is not implemented here — see this package's README "Known
 * limitations": that needs real structural/AST analysis this v1 engine
 * doesn't do, not just line-level pattern matching.
 *
 * `ReviewPolicy` is `@debuggatha/knowledge-system`'s resolved policy
 * (`{ id, rules, packRefs, conflicts }`), not review-engine's bare
 * `ReviewPolicyReference { id }` — see docs/adr/0001 and Architecture doc
 * §4 "Skills".
 */
export function reviewArchitecture(rootDir: string, policy: ReviewPolicy): Finding[] {
  const units = walkSourceUnits(rootDir);
  const ruleFindings = runReviewSkillsEngine(policy, units);
  const analysisFindings = buildAnalysisFindings(rootDir);
  return [...ruleFindings, ...analysisFindings];
}
