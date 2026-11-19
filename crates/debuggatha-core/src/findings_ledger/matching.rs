//! Matching a reported finding to a ledger entry.

use super::entry::LedgerEntry;
use super::fingerprint::compute_finding_fingerprint;
use crate::review_engine::Finding;

/// What matching a reported finding against the ledger found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MatchOutcome {
    /// It is the same finding as an existing entry.
    Matched {
        /// The entry it matched.
        entry_id: String,
        /// Whether the flagged code changed since the entry was last seen.
        changed: bool,
    },
    /// No entry is the same finding.
    New,
}

/// Primary key is (file, rule id); without a rule id it falls back to (file, category).
/// `changed` needs a content anchor on both sides, otherwise a match is reported unchanged.
/// `entries` is every entry, not only active ones: matching a resolved one is what reopens it.
#[must_use]
pub fn default_finding_matcher(candidate: &Finding, entries: &[LedgerEntry]) -> MatchOutcome {
    let fp = compute_finding_fingerprint(candidate);
    for entry in entries {
        if entry.fingerprint.file != fp.file {
            continue;
        }
        let same_identity = match &fp.rule_id {
            Some(rule_id) => entry.fingerprint.rule_id.as_ref() == Some(rule_id),
            None => {
                entry.fingerprint.rule_id.is_none() && entry.fingerprint.category == fp.category
            }
        };
        if !same_identity {
            continue;
        }
        let changed = match (&entry.fingerprint.content_anchor, &fp.content_anchor) {
            (Some(old), Some(new)) => old != new,
            _ => false,
        };
        return MatchOutcome::Matched {
            entry_id: entry.id.clone(),
            changed,
        };
    }
    MatchOutcome::New
}
