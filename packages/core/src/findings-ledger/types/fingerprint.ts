import {
  type Category,
  type Evidence,
  type Finding,
  ruleIdFromEvidence,
} from "../../review-engine/index.js";
import { sha256Hex } from "../internal/hash.js";

/**
 * What Finding Matching keys off of (epic §2, §6) — deliberately never
 * line number alone. `file` + `ruleId` (when the evidence names one) is
 * the primary identity key; `contentAnchor` (a hash of a code excerpt,
 * when evidence provides one) detects whether the same identity's
 * underlying content actually changed between review runs.
 *
 * A multi-location `Finding` is fingerprinted on its *first* location
 * only — see this package's README "Technical decisions" for why that's
 * an accepted v1 simplification, not an oversight.
 */
export interface FindingFingerprint {
  file: string;
  ruleId: string | undefined;
  category: Category;
  contentAnchor: string | undefined;
}

function extractRuleId(evidence: Evidence[]): string | undefined {
  return ruleIdFromEvidence(evidence);
}

function extractContentAnchor(evidence: Evidence[]): string | undefined {
  for (const item of evidence) {
    if (item.kind === "code" && item.excerpt !== undefined) {
      return sha256Hex(item.excerpt).slice(0, 16);
    }
  }
  return undefined;
}

export function computeFindingFingerprint(finding: Finding): FindingFingerprint {
  return {
    file: finding.locations[0]?.file ?? "",
    ruleId: extractRuleId(finding.evidence),
    category: finding.category,
    contentAnchor: extractContentAnchor(finding.evidence),
  };
}
