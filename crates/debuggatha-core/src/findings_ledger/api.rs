use super::entry::{
    HistoryEvent, Ledger, LedgerEntry, LedgerError, TransitionOrigin, transition_finding,
};
use super::lifecycle::LifecycleStatus;
use crate::env::Env;
use crate::review_engine::Severity;

#[derive(Debug, Clone, Default)]
pub struct LedgerEntryFilter {
    pub status: Option<Vec<LifecycleStatus>>,
    pub severity: Option<Vec<Severity>>,
    pub category: Option<Vec<String>>,
    pub file: Option<String>,
}

#[must_use]
pub fn get_entry<'a>(ledger: &'a Ledger, entry_id: &str) -> Option<&'a LedgerEntry> {
    ledger.entries.iter().find(|entry| entry.id == entry_id)
}

#[must_use]
pub fn list_entries<'a>(ledger: &'a Ledger, filter: &LedgerEntryFilter) -> Vec<&'a LedgerEntry> {
    ledger
        .entries
        .iter()
        .filter(|e| filter.status.as_ref().is_none_or(|v| v.contains(&e.status)))
        .filter(|e| {
            filter
                .severity
                .as_ref()
                .is_none_or(|v| v.contains(&e.latest_finding.severity))
        })
        .filter(|e| {
            filter
                .category
                .as_ref()
                .is_none_or(|v| v.contains(&e.latest_finding.category))
        })
        .filter(|e| {
            filter
                .file
                .as_ref()
                .is_none_or(|f| *f == e.fingerprint.file)
        })
        .collect()
}

/// # Errors
/// `EntryNotFound` for an unknown id.
pub fn get_history<'a>(
    ledger: &'a Ledger,
    entry_id: &str,
) -> Result<&'a [HistoryEvent], LedgerError> {
    get_entry(ledger, entry_id)
        .map(|entry| entry.history.as_slice())
        .ok_or_else(|| LedgerError::EntryNotFound(entry_id.to_owned()))
}

/// # Errors
/// `EntryNotFound` for an unknown id, `IllegalTransition` when the lifecycle forbids the move.
pub fn update_finding_status(
    ledger: &Ledger,
    entry_id: &str,
    to: LifecycleStatus,
    origin: TransitionOrigin,
    comment: Option<String>,
    env: &dyn Env,
) -> Result<Ledger, LedgerError> {
    let entry = get_entry(ledger, entry_id)
        .ok_or_else(|| LedgerError::EntryNotFound(entry_id.to_owned()))?;
    let updated = transition_finding(entry, to, origin, comment, env)?;
    let mut next = ledger.clone();
    for existing in &mut next.entries {
        if existing.id == entry_id {
            *existing = updated.clone();
        }
    }
    next.updated_at = env.now();
    Ok(next)
}
