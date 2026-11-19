use std::collections::{HashMap, HashSet};

use serde::Serialize;

use super::entry::{
    Ledger, LedgerEntry, LedgerError, ReviewAssociation, TransitionOrigin, create_ledger_entry,
    refresh_ledger_entry, transition_finding,
};
use super::fingerprint::compute_finding_fingerprint;
use super::lifecycle::{LifecycleStatus, is_active_finding_status};
use super::matching::{MatchOutcome, default_finding_matcher};
use crate::env::Env;
use crate::review_engine::ReviewResult;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncScope {
    Workspace,
    Files(Vec<String>),
}

impl SyncScope {
    fn contains(&self, file: &str) -> bool {
        match self {
            Self::Workspace => true,
            Self::Files(files) => files.iter().any(|f| f == file),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncReport {
    pub new_entry_ids: Vec<String>,
    pub unchanged_entry_ids: Vec<String>,
    pub changed_entry_ids: Vec<String>,
    pub auto_resolved_entry_ids: Vec<String>,
    pub reopened_entry_ids: Vec<String>,
}

/// Fold a finished review into the ledger. Findings no longer detected inside `scope` resolve;
/// resolved or dismissed ones that reappear reopen. The input ledger is not modified.
///
/// `association` is built by the caller from the review and the repository snapshot.
///
/// # Errors
/// Only if an internal transition is refused, which the lifecycle table rules out.
pub fn synchronize_review_result(
    ledger: &Ledger,
    result: &ReviewResult,
    association: &ReviewAssociation,
    scope: &SyncScope,
    env: &dyn Env,
) -> Result<(Ledger, SyncReport), LedgerError> {
    let mut entries: Vec<LedgerEntry> = ledger.entries.clone();
    let mut index: HashMap<String, usize> = entries
        .iter()
        .enumerate()
        .map(|(i, e)| (e.id.clone(), i))
        .collect();
    let mut touched: HashSet<String> = HashSet::new();
    let mut report = SyncReport::default();
    let review_origin = || TransitionOrigin::Review {
        session_id: result.session_id.clone(),
    };

    for finding in &result.findings {
        let fingerprint = compute_finding_fingerprint(finding);
        match default_finding_matcher(finding, &entries) {
            MatchOutcome::New => {
                let entry = create_ledger_entry(
                    fingerprint,
                    finding.clone(),
                    association.clone(),
                    review_origin(),
                    env,
                );
                touched.insert(entry.id.clone());
                report.new_entry_ids.push(entry.id.clone());
                index.insert(entry.id.clone(), entries.len());
                entries.push(entry);
            }
            MatchOutcome::Matched { entry_id, changed } => {
                let Some(&at) = index.get(&entry_id) else {
                    continue;
                };
                touched.insert(entry_id.clone());
                let existing = &entries[at];
                if matches!(
                    existing.status,
                    LifecycleStatus::Resolved | LifecycleStatus::Dismissed
                ) {
                    let reopened = transition_finding(
                        existing,
                        LifecycleStatus::Reopened,
                        review_origin(),
                        None,
                        env,
                    )?;
                    entries[at] =
                        refresh_ledger_entry(&reopened, finding.clone(), fingerprint, association);
                    report.reopened_entry_ids.push(entry_id);
                } else {
                    entries[at] =
                        refresh_ledger_entry(existing, finding.clone(), fingerprint, association);
                    if changed {
                        report.changed_entry_ids.push(entry_id);
                    } else {
                        report.unchanged_entry_ids.push(entry_id);
                    }
                }
            }
        }
    }

    for entry in &mut entries {
        if !is_active_finding_status(entry.status)
            || touched.contains(&entry.id)
            || !scope.contains(&entry.fingerprint.file)
        {
            continue;
        }
        *entry = transition_finding(entry, LifecycleStatus::Resolved, review_origin(), None, env)?;
        report.auto_resolved_entry_ids.push(entry.id.clone());
    }

    let mut next = ledger.clone();
    next.entries = entries;
    next.updated_at = env.now();
    Ok((next, report))
}
