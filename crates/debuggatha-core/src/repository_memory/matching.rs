//! Whether a memory item covers a finding.

use super::item::{MemoryDetail, MemoryItem};
use crate::findings_ledger::compute_finding_fingerprint;
use crate::review_engine::Finding;

/// A suppression is keyed on the same identity the ledger uses: file plus rule id, or file plus
/// category for a finding that cites no rule.
#[must_use]
pub fn suppression_matches(item: &MemoryItem, finding: &Finding) -> bool {
    let MemoryDetail::Suppression {
        fingerprint_file,
        fingerprint_rule_id,
        fingerprint_category,
    } = &item.detail
    else {
        return false;
    };
    let fp = compute_finding_fingerprint(finding);
    if fp.file != *fingerprint_file {
        return false;
    }
    match fingerprint_rule_id {
        Some(rule_id) => fp.rule_id.as_ref() == Some(rule_id),
        None => fp.rule_id.is_none() && fp.category == *fingerprint_category,
    }
}

/// Whether `item` is an accepted deviation that covers `finding`: the same rule, in the file or anywhere when the item names none.
#[must_use]
pub fn accepted_deviation_matches(item: &MemoryItem, finding: &Finding) -> bool {
    let MemoryDetail::AcceptedDeviation {
        rule_id,
        scope_file,
    } = &item.detail
    else {
        return false;
    };
    let fp = compute_finding_fingerprint(finding);
    if fp.rule_id.as_ref() != Some(rule_id) {
        return false;
    }
    scope_file.as_ref().is_none_or(|file| fp.file == *file)
}

/// Whether `item` is an exception that covers `finding`: the same rule when it names one, in the file or anywhere when it names none.
#[must_use]
pub fn exception_matches(item: &MemoryItem, finding: &Finding) -> bool {
    let MemoryDetail::Exception {
        rule_id,
        scope_file,
        ..
    } = &item.detail
    else {
        return false;
    };
    let fp = compute_finding_fingerprint(finding);
    if rule_id
        .as_ref()
        .is_some_and(|wanted| fp.rule_id.as_ref() != Some(wanted))
    {
        return false;
    }
    scope_file.as_ref().is_none_or(|file| fp.file == *file)
}
