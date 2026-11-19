//! A memory item: a structured engineering decision, its evidence and its history.

use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::lifecycle::{MemoryStatus, can_transition_memory};
use crate::env::Env;

/// How well established a memory item is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum MemoryConfidence {
    /// Raised by a detector; nobody has checked it.
    Inferred,
    /// Backed by something the repository itself writes down.
    Documented,
    /// A person confirmed it.
    UserConfirmed,
}

/// Who or what caused a memory item to change state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(
    tag = "kind",
    rename_all = "lowercase",
    rename_all_fields = "camelCase"
)]
pub enum MemoryOrigin {
    /// A review that detected it.
    Detection {
        /// The review session.
        session_id: String,
    },
    /// A person.
    User {
        /// Who, as the person gave it.
        actor: String,
    },
}

/// One change of a memory item's state, kept for the audit trail.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct MemoryHistoryEvent {
    /// When it happened, ISO 8601.
    pub timestamp: String,
    /// The state before; `None` for the creation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_state: Option<MemoryStatus>,
    /// The state after.
    pub new_state: MemoryStatus,
    /// What caused it.
    pub origin: MemoryOrigin,
    /// The reason given, when there is one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
}

/// A file and what it shows, as the basis for a memory item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct MemoryEvidence {
    /// Path relative to the repository root.
    pub file: String,
    /// What it shows, in a sentence.
    pub detail: String,
}

/// The kind of exception an item records.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum ExceptionKind {
    /// Code crosses a layer boundary on purpose.
    LayerCrossing,
    /// A workaround for another system's behavior.
    CompatibilityWorkaround,
    /// Integration with an old system the repository keeps.
    LegacyIntegration,
    /// Any other deliberate exception.
    Other,
}

/// What kind of decision an item records, and the fields that kind needs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(
    tag = "category",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase"
)]
pub enum MemoryDetail {
    /// A rule the repository accepts breaking, here.
    AcceptedDeviation {
        /// The rule accepted.
        rule_id: String,
        /// The file it is accepted in; `None` for the whole repository.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        scope_file: Option<String>,
    },
    /// A finding the repository accepted, matched by its fingerprint.
    Suppression {
        /// The file of the accepted finding.
        fingerprint_file: String,
        /// The rule of the accepted finding, when it has one.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        fingerprint_rule_id: Option<String>,
        /// The category of the accepted finding.
        fingerprint_category: String,
    },
    /// A deliberate exception to how the code is organized.
    Exception {
        /// What kind of exception.
        exception_kind: ExceptionKind,
        /// The rule it relates to, when it has one.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        rule_id: Option<String>,
        /// The file it applies to; `None` for the whole repository.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        scope_file: Option<String>,
    },
    /// A convention the repository follows.
    Convention {
        /// The convention, in a sentence.
        rule: String,
    },
    /// A note about a ledger entry's history.
    ReviewHistory {
        /// The ledger entry.
        ledger_entry_id: String,
        /// What happened to it.
        note: String,
    },
    /// A decision the repository recorded.
    Decision {
        /// The decision, in a sentence.
        decision: String,
        /// The document that records it.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        documentation_ref: Option<String>,
    },
}

/// A structured fact about how the repository is engineered, with its own lifecycle; only `active` items ever filter findings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct MemoryItem {
    /// Stable identifier.
    pub id: String,
    /// Where it is in the lifecycle.
    pub status: MemoryStatus,
    /// How well established it is.
    pub confidence: MemoryConfidence,
    /// Every state change, oldest first.
    pub history: Vec<MemoryHistoryEvent>,
    /// When it was created, ISO 8601.
    pub created_at: String,
    /// When it last changed, ISO 8601.
    pub updated_at: String,
    /// Why the repository does this.
    pub rationale: String,
    /// What shows it.
    pub evidence: Vec<MemoryEvidence>,
    /// What kind of decision it is, with the fields that kind needs.
    #[serde(flatten)]
    pub detail: MemoryDetail,
}

/// What is needed to create a memory item; it always starts as `detected`.
pub struct NewMemoryItem {
    /// How well established it is.
    pub confidence: MemoryConfidence,
    /// Why the repository does this.
    pub rationale: String,
    /// What shows it.
    pub evidence: Vec<MemoryEvidence>,
    /// What kind of decision it is.
    pub detail: MemoryDetail,
    /// Who or what proposed it.
    pub origin: MemoryOrigin,
}

/// A new item always starts as `detected`; nothing is active until a person says so.
#[must_use]
pub fn create_memory_item(input: NewMemoryItem, env: &dyn Env) -> MemoryItem {
    let now = env.now();
    MemoryItem {
        id: env.new_id(),
        status: MemoryStatus::Detected,
        confidence: input.confidence,
        history: vec![MemoryHistoryEvent {
            timestamp: now.clone(),
            previous_state: None,
            new_state: MemoryStatus::Detected,
            origin: input.origin,
            comment: None,
        }],
        created_at: now.clone(),
        updated_at: now,
        rationale: input.rationale,
        evidence: input.evidence,
        detail: input.detail,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MemoryTransitionError {
    Illegal {
        from: MemoryStatus,
        to: MemoryStatus,
    },
}

impl fmt::Display for MemoryTransitionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self::Illegal { from, to } = self;
        write!(
            f,
            "Illegal memory lifecycle transition: \"{}\" -> \"{}\".",
            from.as_str(),
            to.as_str()
        )
    }
}

/// # Errors
/// `Illegal` when the lifecycle does not allow `item.status -> to`.
pub fn transition_memory(
    item: &MemoryItem,
    to: MemoryStatus,
    origin: MemoryOrigin,
    comment: Option<String>,
    env: &dyn Env,
) -> Result<MemoryItem, MemoryTransitionError> {
    if !can_transition_memory(item.status, to) {
        return Err(MemoryTransitionError::Illegal {
            from: item.status,
            to,
        });
    }
    let timestamp = env.now();
    let mut next = item.clone();
    next.history.push(MemoryHistoryEvent {
        timestamp: timestamp.clone(),
        previous_state: Some(item.status),
        new_state: to,
        origin,
        comment,
    });
    next.status = to;
    next.updated_at = timestamp;
    Ok(next)
}
