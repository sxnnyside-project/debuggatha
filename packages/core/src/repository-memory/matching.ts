import { computeFindingFingerprint } from "../findings-ledger/index.js";
import type { Finding } from "../review-engine/index.js";
import type { AcceptedDeviationItem, ExceptionItem, SuppressionItem } from "./types/item.js";

/**
 * Matching reuses `core/findings-ledger`'s own
 * `computeFindingFingerprint` rather than re-deriving a finding's
 * identity — "Repository Memory should enrich the Findings Ledger. It
 * should not duplicate it." A suppression/deviation/exception matches a
 * `Finding` by the exact same (file, ruleId-or-category) identity the
 * Ledger already uses to recognize "the same finding" across review
 * runs.
 */
export function suppressionMatches(item: SuppressionItem, finding: Finding): boolean {
  const fp = computeFindingFingerprint(finding);
  if (fp.file !== item.fingerprintFile) return false;
  if (item.fingerprintRuleId !== undefined) return fp.ruleId === item.fingerprintRuleId;
  return fp.ruleId === undefined && fp.category === item.fingerprintCategory;
}

export function acceptedDeviationMatches(item: AcceptedDeviationItem, finding: Finding): boolean {
  const fp = computeFindingFingerprint(finding);
  if (fp.ruleId !== item.ruleId) return false;
  return item.scopeFile === undefined || fp.file === item.scopeFile;
}

export function exceptionMatches(item: ExceptionItem, finding: Finding): boolean {
  const fp = computeFindingFingerprint(finding);
  if (item.ruleId !== undefined && fp.ruleId !== item.ruleId) return false;
  return item.scopeFile === undefined || fp.file === item.scopeFile;
}
