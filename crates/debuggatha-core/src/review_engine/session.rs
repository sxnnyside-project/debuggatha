use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::finding::{PackReference, PolicyReference};
use super::lifecycle::{ReviewLifecycleStatus, can_transition};
use super::request::ReviewRequest;
use super::result::{ExecutionMetadata, ReviewSource};
use crate::env::Env;
use crate::repository_intelligence::RepositoryContext;

/// The traceable record of one review in progress: the (immutable) repository context, which
/// policy and packs were selected, and the lifecycle status.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReviewSession {
    pub id: String,
    #[schemars(extend("format" = "date-time"))]
    pub created_at: String,
    pub request: ReviewRequest,
    pub repository_context: RepositoryContext,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_policy: Option<PolicyReference>,
    pub selected_packs: Vec<PackReference>,
    pub status: ReviewLifecycleStatus,
    pub execution: ExecutionMetadata,
}

pub struct CreateReviewSessionInput {
    pub id: Option<String>,
    pub created_at: Option<String>,
    pub request: ReviewRequest,
    pub repository_context: RepositoryContext,
    pub selected_policy: Option<PolicyReference>,
    pub selected_packs: Vec<PackReference>,
}

#[must_use]
pub fn create_review_session(input: CreateReviewSessionInput, env: &dyn Env) -> ReviewSession {
    ReviewSession {
        id: input.id.unwrap_or_else(|| env.new_id()),
        created_at: input.created_at.unwrap_or_else(|| env.now()),
        request: input.request,
        repository_context: input.repository_context,
        selected_policy: input.selected_policy,
        selected_packs: input.selected_packs,
        status: ReviewLifecycleStatus::Requested,
        execution: ExecutionMetadata::default(),
    }
}

/// Fields a transition may set. A field left `None` keeps its value; the engine never clears one.
#[derive(Debug, Clone, Default)]
pub struct ExecutionPatch {
    pub source: Option<ReviewSource>,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub duration_ms: Option<u64>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IllegalTransition {
    pub from: ReviewLifecycleStatus,
    pub to: ReviewLifecycleStatus,
}

impl fmt::Display for IllegalTransition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Illegal review lifecycle transition: \"{}\" -> \"{}\".",
            self.from.as_str(),
            self.to.as_str()
        )
    }
}

impl std::error::Error for IllegalTransition {}

/// Pure: consumes the session and returns the next one.
///
/// # Errors
/// Any transition `can_transition` does not allow; the session is dropped with it, so the caller
/// keeps a clone if it needs to continue.
pub fn transition_session(
    session: ReviewSession,
    to: ReviewLifecycleStatus,
    patch: ExecutionPatch,
) -> Result<ReviewSession, IllegalTransition> {
    if !can_transition(session.status, to) {
        return Err(IllegalTransition {
            from: session.status,
            to,
        });
    }
    let e = session.execution;
    Ok(ReviewSession {
        status: to,
        execution: ExecutionMetadata {
            source: patch.source.or(e.source),
            started_at: patch.started_at.or(e.started_at),
            completed_at: patch.completed_at.or(e.completed_at),
            duration_ms: patch.duration_ms.or(e.duration_ms),
            error: patch.error.or(e.error),
        },
        ..session
    })
}
