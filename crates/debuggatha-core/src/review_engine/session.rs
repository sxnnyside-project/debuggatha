//! A review session: the request, the repository, the policy and the lifecycle status.

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
    /// Stable identifier of the session.
    pub id: String,
    /// When it was created, ISO 8601.
    #[schemars(extend("format" = "date-time"))]
    pub created_at: String,
    /// What was asked.
    pub request: ReviewRequest,
    /// The repository as it was when the session started.
    pub repository_context: RepositoryContext,
    /// The policy chosen for the review.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_policy: Option<PolicyReference>,
    /// The packs chosen for the review.
    pub selected_packs: Vec<PackReference>,
    /// Where the session is in its lifecycle.
    pub status: ReviewLifecycleStatus,
    /// Timing and outcome so far.
    pub execution: ExecutionMetadata,
}

/// What is needed to create a session; the id and time are generated when absent.
pub struct CreateReviewSessionInput {
    /// The id to use, or `None` to generate one.
    pub id: Option<String>,
    /// The creation time to use, or `None` for now.
    pub created_at: Option<String>,
    /// What was asked.
    pub request: ReviewRequest,
    /// The repository snapshot.
    pub repository_context: RepositoryContext,
    /// The policy chosen.
    pub selected_policy: Option<PolicyReference>,
    /// The packs chosen.
    pub selected_packs: Vec<PackReference>,
}

/// A session in the `requested` status, with an id and a time from `env` when the input gives none.
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
    /// What ran the review.
    pub source: Option<ReviewSource>,
    /// When it started, ISO 8601.
    pub started_at: Option<String>,
    /// When it ended, ISO 8601.
    pub completed_at: Option<String>,
    /// How long it took, in milliseconds.
    pub duration_ms: Option<u64>,
    /// The failure, when the review failed.
    pub error: Option<String>,
}

/// A move the lifecycle does not allow.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IllegalTransition {
    /// The status the session was in.
    pub from: ReviewLifecycleStatus,
    /// The status asked for.
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
