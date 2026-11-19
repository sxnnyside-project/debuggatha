//! What a review is asked to cover.

use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::env::Env;

/// How much a review does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ReviewDepth {
    /// The built-in detectors only.
    Quick,
    /// The detectors and the installed analyzers.
    Full,
    /// Everything, and the whole repository's architecture.
    Architectural,
}

/// One scope model: a diff, a list of files and a workspace review differ only in *what* is
/// reviewed, not in what a review request is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum ReviewScope {
    /// No `base` means "since the last commit".
    Diff {
        /// The base to diff against.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        base: Option<String>,
    },
    /// A list of files.
    Files {
        /// At least one; `create_review_request` refuses an empty list.
        #[schemars(length(min = 1))]
        paths: Vec<String>,
    },
    /// The whole repository.
    Workspace,
}

/// What a review is asked to cover.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReviewRequest {
    /// Stable identifier of the request.
    pub id: String,
    /// What to review.
    pub scope: ReviewScope,
    /// How much to do.
    pub depth: ReviewDepth,
    /// When it was requested, ISO 8601.
    #[schemars(extend("format" = "date-time"))]
    pub requested_at: String,
    /// Packs to add on top of the detected ones.
    pub requested_pack_ids: Vec<String>,
    /// A policy id a previous review reported.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requested_policy_id: Option<String>,
}

/// What is needed to create a request; the id and time are generated when absent.
pub struct CreateReviewRequestInput {
    /// The id to use, or `None` to generate one.
    pub id: Option<String>,
    /// What to review.
    pub scope: ReviewScope,
    /// How much to do.
    pub depth: ReviewDepth,
    /// The time to use, or `None` for now.
    pub requested_at: Option<String>,
    /// Packs to add.
    pub requested_pack_ids: Vec<String>,
    /// A policy id to assert.
    pub requested_policy_id: Option<String>,
}

impl CreateReviewRequestInput {
    /// A request for `scope` and `depth` with nothing else asked.
    #[must_use]
    pub fn new(scope: ReviewScope, depth: ReviewDepth) -> Self {
        Self {
            id: None,
            scope,
            depth,
            requested_at: None,
            requested_pack_ids: Vec::new(),
            requested_policy_id: None,
        }
    }
}

/// A files scope with no paths; at least one is required.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmptyFileScope;

impl fmt::Display for EmptyFileScope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("A file review request must name at least one file.")
    }
}

impl std::error::Error for EmptyFileScope {}

/// # Errors
/// A file review that names no file is refused.
pub fn create_review_request(
    input: CreateReviewRequestInput,
    env: &dyn Env,
) -> Result<ReviewRequest, EmptyFileScope> {
    if matches!(&input.scope, ReviewScope::Files { paths } if paths.is_empty()) {
        return Err(EmptyFileScope);
    }
    Ok(ReviewRequest {
        id: input.id.unwrap_or_else(|| env.new_id()),
        scope: input.scope,
        depth: input.depth,
        requested_at: input.requested_at.unwrap_or_else(|| env.now()),
        requested_pack_ids: input.requested_pack_ids,
        requested_policy_id: input.requested_policy_id,
    })
}

#[cfg(test)]
// A failed expect is a failed test, which is the intent here.
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use crate::env::SystemEnv;
    use serde_json::json;

    fn make(scope: ReviewScope, depth: ReviewDepth) -> Result<ReviewRequest, EmptyFileScope> {
        create_review_request(CreateReviewRequestInput::new(scope, depth), &SystemEnv)
    }

    #[test]
    fn the_three_scopes_share_one_request_shape() {
        let diff = make(
            ReviewScope::Diff {
                base: Some("main".into()),
            },
            ReviewDepth::Full,
        );
        let files = make(
            ReviewScope::Files {
                paths: vec!["src/index.ts".into()],
            },
            ReviewDepth::Quick,
        );
        let ws = make(ReviewScope::Workspace, ReviewDepth::Architectural);
        for r in [diff, files, ws] {
            let v = serde_json::to_value(r.expect("valid")).expect("serializes");
            for key in ["id", "scope", "depth", "requestedAt", "requestedPackIds"] {
                assert!(v.get(key).is_some(), "missing {key}");
            }
            assert!(v.get("requestedPolicyId").is_none());
        }
    }

    #[test]
    fn diff_without_base_omits_the_key() {
        let r = make(ReviewScope::Diff { base: None }, ReviewDepth::Quick).expect("valid");
        assert_eq!(
            serde_json::to_value(&r.scope).expect("ok"),
            json!({ "kind": "diff" })
        );
    }

    #[test]
    fn empty_file_review_is_refused_with_the_typescript_message() {
        let err = make(ReviewScope::Files { paths: vec![] }, ReviewDepth::Quick)
            .expect_err("must refuse");
        assert_eq!(
            err.to_string(),
            "A file review request must name at least one file."
        );
    }

    #[test]
    fn requested_at_is_generated_when_absent() {
        let r = make(ReviewScope::Workspace, ReviewDepth::Full).expect("valid");
        assert!(r.requested_at.ends_with('Z') && r.requested_at.len() == 24);
    }
}
