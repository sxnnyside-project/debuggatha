use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::lifecycle::{MemoryStatus, can_transition_memory};
use crate::env::Env;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum MemoryConfidence {
    Inferred,
    Documented,
    UserConfirmed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(
    tag = "kind",
    rename_all = "lowercase",
    rename_all_fields = "camelCase"
)]
pub enum MemoryOrigin {
    Detection { session_id: String },
    User { actor: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct MemoryHistoryEvent {
    pub timestamp: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_state: Option<MemoryStatus>,
    pub new_state: MemoryStatus,
    pub origin: MemoryOrigin,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct MemoryEvidence {
    pub file: String,
    pub detail: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum ExceptionKind {
    LayerCrossing,
    CompatibilityWorkaround,
    LegacyIntegration,
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
    AcceptedDeviation {
        rule_id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        scope_file: Option<String>,
    },
    Suppression {
        fingerprint_file: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        fingerprint_rule_id: Option<String>,
        fingerprint_category: String,
    },
    Exception {
        exception_kind: ExceptionKind,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        rule_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        scope_file: Option<String>,
    },
    Convention {
        rule: String,
    },
    ReviewHistory {
        ledger_entry_id: String,
        note: String,
    },
    Decision {
        decision: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        documentation_ref: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct MemoryItem {
    pub id: String,
    pub status: MemoryStatus,
    pub confidence: MemoryConfidence,
    pub history: Vec<MemoryHistoryEvent>,
    pub created_at: String,
    pub updated_at: String,
    pub rationale: String,
    pub evidence: Vec<MemoryEvidence>,
    #[serde(flatten)]
    pub detail: MemoryDetail,
}

pub struct NewMemoryItem {
    pub confidence: MemoryConfidence,
    pub rationale: String,
    pub evidence: Vec<MemoryEvidence>,
    pub detail: MemoryDetail,
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
