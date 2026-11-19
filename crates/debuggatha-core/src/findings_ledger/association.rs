//! What the review that produced an entry used.

use serde_json::{Value, json};

use super::entry::{ReviewAssociation, SnapshotRef};
use crate::canonical::short_hash;
use crate::review_engine::ReviewResult;

/// A reference to the repository state a finding was produced against: when, and a short hash of
/// what the review depended on (stack, dependencies, documentation, criteria).
///
/// The four parts are the serialized profiles of the repository context.
#[must_use]
pub fn snapshot_ref_for(
    generated_at: &str,
    stack: &Value,
    dependencies: &Value,
    documentation: &Value,
    criteria: &Value,
) -> SnapshotRef {
    SnapshotRef {
        generated_at: generated_at.to_owned(),
        fingerprint: short_hash(&json!({
            "stack": stack,
            "dependencies": dependencies,
            "documentation": documentation,
            "criteria": criteria,
        })),
    }
}

/// What the review that produced `result` used, for the entries it creates or refreshes.
#[must_use]
pub fn association_for(result: &ReviewResult, snapshot: SnapshotRef) -> ReviewAssociation {
    ReviewAssociation {
        created_by_session_id: result.session_id.clone(),
        last_updated_by_session_id: result.session_id.clone(),
        repository_snapshot: snapshot,
        applied_policy: result.summary.applied_policies.first().cloned(),
        applied_packs: result.summary.applied_packs.clone(),
    }
}
