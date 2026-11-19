//! Review types: findings, evidence, recommendations and results (port of `core/review-engine`).

mod finding;
mod lifecycle;
mod request;
mod result;
mod session;

pub use finding::{
    Category, Confidence, CreateFindingInput, EditSafety, Evidence, Finding, FindingError,
    FindingLocation, LineRange, PackReference, PolicyReference, Recommendation,
    RecommendationAction, RecommendationEdit, RecommendationExample, SemanticRole, SemanticVerdict,
    Severity, SourceKind, affected_files, create_finding, rule_id_from_evidence,
};
pub use lifecycle::{ReviewLifecycleStatus, can_transition, is_terminal};
pub use request::{
    CreateReviewRequestInput, EmptyFileScope, ReviewDepth, ReviewRequest, ReviewScope,
    create_review_request,
};
pub use result::{
    ExecutionMetadata, ReviewResult, ReviewSource, ReviewSummary, summarize_findings,
};
pub use session::{
    CreateReviewSessionInput, ExecutionPatch, IllegalTransition, ReviewSession,
    create_review_session, transition_session,
};
