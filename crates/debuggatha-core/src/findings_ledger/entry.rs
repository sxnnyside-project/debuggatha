//! Ledger entries and the ledger file.

use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::fingerprint::FindingFingerprint;
use super::lifecycle::{LifecycleStatus, can_transition_finding};
use crate::env::Env;
use crate::review_engine::{Finding, PackReference, PolicyReference};

/// Version of the ledger file format; a file with another version is refused.
pub const CURRENT_SCHEMA_VERSION: u32 = 1;

/// What caused a ledger entry to change state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(
    tag = "kind",
    rename_all = "lowercase",
    rename_all_fields = "camelCase"
)]
pub enum TransitionOrigin {
    /// A review.
    Review {
        /// The review session.
        session_id: String,
    },
    /// A person.
    Manual {
        /// Who, as the person gave it.
        actor: String,
    },
}

/// One change of a finding's state, kept as an append-only history.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEvent {
    /// When it happened, ISO 8601.
    pub timestamp: String,
    /// The state before; `None` for the creation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_state: Option<LifecycleStatus>,
    /// The state after.
    pub new_state: LifecycleStatus,
    /// What caused it.
    pub origin: TransitionOrigin,
    /// The reason given, when there is one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
}

/// The repository snapshot a review ran against.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotRef {
    /// When the snapshot was built, ISO 8601.
    pub generated_at: String,
    /// Fingerprint of the snapshot.
    pub fingerprint: String,
}

/// Which review touched an entry, with what.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReviewAssociation {
    /// The review that first reported it.
    pub created_by_session_id: String,
    /// The review that last saw it.
    pub last_updated_by_session_id: String,
    /// The snapshot of the latest review.
    pub repository_snapshot: SnapshotRef,
    /// The policy the latest review applied.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub applied_policy: Option<PolicyReference>,
    /// The packs it applied.
    pub applied_packs: Vec<PackReference>,
}

/// One finding the ledger tracks: its identity, state, history and latest form.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct LedgerEntry {
    /// Stable identifier.
    pub id: String,
    /// What makes it the same finding across reviews: file, rule and content, never the line alone.
    pub fingerprint: FindingFingerprint,
    /// Where it is in the lifecycle.
    pub status: LifecycleStatus,
    /// Every state change, oldest first.
    pub history: Vec<HistoryEvent>,
    /// The finding as the latest review reported it.
    pub latest_finding: Finding,
    /// What the reviews that saw it used.
    pub review: ReviewAssociation,
}

/// The persistent record of findings in `.debuggatha/ledger.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Ledger {
    /// Version of the file format.
    #[schemars(range(min = 1))]
    pub schema_version: u32,
    /// The repository the ledger belongs to.
    pub repository_root: String,
    /// Every finding it tracks.
    pub entries: Vec<LedgerEntry>,
    /// When it was created, ISO 8601.
    pub created_at: String,
    /// When it last changed, ISO 8601.
    pub updated_at: String,
}

/// Why a ledger operation failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LedgerError {
    /// The lifecycle does not allow the move.
    IllegalTransition {
        /// The state it is in.
        from: LifecycleStatus,
        /// The state asked for.
        to: LifecycleStatus,
    },
    /// No entry has that id.
    EntryNotFound(String),
}

impl fmt::Display for LedgerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::IllegalTransition { from, to } => write!(
                f,
                "Illegal finding lifecycle transition: \"{}\" -> \"{}\".",
                from.as_str(),
                to.as_str()
            ),
            Self::EntryNotFound(id) => write!(f, "No ledger entry with id \"{id}\"."),
        }
    }
}

impl std::error::Error for LedgerError {}

/// A ledger with no entries, stamped with the current time.
#[must_use]
pub fn create_empty_ledger(repository_root: &str, env: &dyn Env) -> Ledger {
    let now = env.now();
    Ledger {
        schema_version: CURRENT_SCHEMA_VERSION,
        repository_root: repository_root.to_owned(),
        entries: Vec::new(),
        created_at: now.clone(),
        updated_at: now,
    }
}

/// A new entry for a finding the first time a review reports it: status `open`, history of one event.
#[must_use]
pub fn create_ledger_entry(
    fingerprint: FindingFingerprint,
    finding: Finding,
    review: ReviewAssociation,
    origin: TransitionOrigin,
    env: &dyn Env,
) -> LedgerEntry {
    LedgerEntry {
        id: env.new_id(),
        fingerprint,
        status: LifecycleStatus::Open,
        history: vec![HistoryEvent {
            timestamp: env.now(),
            previous_state: None,
            new_state: LifecycleStatus::Open,
            origin,
            comment: None,
        }],
        latest_finding: finding,
        review,
    }
}

/// # Errors
/// `IllegalTransition` when the lifecycle does not allow `entry.status -> to`.
pub fn transition_finding(
    entry: &LedgerEntry,
    to: LifecycleStatus,
    origin: TransitionOrigin,
    comment: Option<String>,
    env: &dyn Env,
) -> Result<LedgerEntry, LedgerError> {
    if !can_transition_finding(entry.status, to) {
        return Err(LedgerError::IllegalTransition {
            from: entry.status,
            to,
        });
    }
    let mut next = entry.clone();
    next.history.push(HistoryEvent {
        timestamp: env.now(),
        previous_state: Some(entry.status),
        new_state: to,
        origin,
        comment,
    });
    next.status = to;
    Ok(next)
}

/// The entry after a review sees the same finding again: the latest finding, fingerprint and session are replaced, the history is kept.
#[must_use]
pub fn refresh_ledger_entry(
    entry: &LedgerEntry,
    finding: Finding,
    fingerprint: FindingFingerprint,
    review: &ReviewAssociation,
) -> LedgerEntry {
    let mut next = entry.clone();
    next.fingerprint = fingerprint;
    next.latest_finding = finding;
    next.review
        .last_updated_by_session_id
        .clone_from(&review.last_updated_by_session_id);
    next
}
