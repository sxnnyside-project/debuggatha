/** Review Skills and Analysis Skills. */

export { buildAnalysisFindings } from "./engine/analysis-findings.js";
export type { DetectorHit, DetectorInput, RuleDetector } from "./engine/detectors.js";
// The unified engine underneath the three Review Skills above — exported
// so it can be unit-tested directly and so other Skills (e.g. future
// Analysis Skills) can reuse the same detector registry without going
// back through a `ReviewPolicy`.
export { detectorFor, RULE_DETECTORS } from "./engine/detectors.js";
export { parseUnifiedDiff } from "./engine/diff-parser.js";
export { readSourceUnits, walkSourceUnits } from "./engine/read-files.js";
export type { SourceUnit } from "./engine/run.js";
export { runReviewSkillsEngine } from "./engine/run.js";
export { reviewArchitecture } from "./review/architecture-review.js";
export { reviewDiff } from "./review/diff-review.js";
export { reviewFiles } from "./review/file-review.js";
