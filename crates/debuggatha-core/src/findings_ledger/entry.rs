use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::fingerprint::FindingFingerprint;
use super::lifecycle::{LifecycleStatus, can_transition_finding};
use crate::env::Env;
use crate::review_engine::{Finding, PackReference, PolicyReference};

pub const CURRENT_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(
    tag = "kind",
    rename_all = "lowercase",
    rename_all_fields = "camelCase"
)]
pub enum TransitionOrigin {
    Review { session_id: String },
    Manual { actor: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEvent {
    pub timestamp: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_state: Option<LifecycleStatus>,
    pub new_state: LifecycleStatus,
    pub origin: TransitionOrigin,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotRef {
    pub generated_at: String,
    pub fingerprint: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReviewAssociation {
    pub created_by_session_id: String,
    pub last_updated_by_session_id: String,
    pub repository_snapshot: SnapshotRef,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub applied_policy: Option<PolicyReference>,
    pub applied_packs: Vec<PackReference>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct LedgerEntry {
    pub id: String,
    pub fingerprint: FindingFingerprint,
    pub status: LifecycleStatus,
    pub history: Vec<HistoryEvent>,
    pub latest_finding: Finding,
    pub review: ReviewAssociation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Ledger {
    #[schemars(range(min = 1))]
    pub schema_version: u32,
    pub repository_root: String,
    pub entries: Vec<LedgerEntry>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LedgerError {
    IllegalTransition {
        from: LifecycleStatus,
        to: LifecycleStatus,
    },
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
