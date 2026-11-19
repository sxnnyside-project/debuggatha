//! The Rust review request, lifecycle and session against the TypeScript reference
//! (`scripts/parity/gen-session-fixtures.ts` at tag `ts-engine-final`).
#![allow(clippy::expect_used, clippy::unwrap_used)] // a failed expect is a failed test

mod common;

use std::collections::BTreeMap;

use debuggatha_core::env::Env;
use debuggatha_core::repository_intelligence::RepositoryContext;
use debuggatha_core::review_engine::{
    CreateReviewRequestInput, CreateReviewSessionInput, ExecutionPatch, ReviewLifecycleStatus,
    ReviewRequest, ReviewScope, ReviewSession, create_review_request, create_review_session,
    transition_session,
};
use serde_json::{Value, json};

struct FixedEnv;
impl Env for FixedEnv {
    fn now(&self) -> String {
        "2026-07-10T00:00:00.000Z".to_owned()
    }
    fn new_id(&self) -> String {
        "r1".to_owned()
    }
}

fn status(name: &str) -> ReviewLifecycleStatus {
    serde_json::from_value(json!(name)).expect("known status")
}

fn request_input(v: &Value) -> CreateReviewRequestInput {
    let scope: ReviewScope = serde_json::from_value(v["scope"].clone()).expect("scope");
    let mut input = CreateReviewRequestInput::new(
        scope,
        serde_json::from_value(v["depth"].clone()).expect("depth"),
    );
    if let Some(ids) = v.get("requestedPackIds") {
        input.requested_pack_ids = serde_json::from_value(ids.clone()).expect("ids");
    }
    input.requested_policy_id = v["requestedPolicyId"].as_str().map(str::to_owned);
    input
}

#[test]
fn requests_match_typescript() {
    let fx = common::fixture("session-conformance.json");
    for case in fx["requests"].as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        let got = create_review_request(request_input(&case["input"]), &FixedEnv);
        match (got, case.get("error")) {
            (Ok(r), None) => {
                assert_eq!(serde_json::to_value(&r).unwrap(), case["request"], "{name}");
            }
            (Err(e), Some(msg)) => assert_eq!(&e.to_string(), msg.as_str().unwrap(), "{name}"),
            (got, want) => panic!("{name}: got {got:?}, expected error {want:?}"),
        }
    }
}

fn base_session(fx: &Value) -> ReviewSession {
    let request: ReviewRequest =
        serde_json::from_value(fx["session"]["request"].clone()).expect("request");
    let context: RepositoryContext =
        serde_json::from_value(fx["context"].clone()).expect("context");
    create_review_session(
        CreateReviewSessionInput {
            id: Some("s1".into()),
            created_at: Some("2026-07-10T00:00:00.000Z".into()),
            request,
            repository_context: context,
            selected_policy: serde_json::from_value(fx["session"]["selectedPolicy"].clone()).ok(),
            selected_packs: serde_json::from_value(fx["session"]["selectedPacks"].clone())
                .expect("packs"),
        },
        &FixedEnv,
    )
}

fn patch(v: Option<&Value>) -> ExecutionPatch {
    let Some(v) = v else {
        return ExecutionPatch::default();
    };
    ExecutionPatch {
        source: v
            .get("source")
            .map(|s| serde_json::from_value(s.clone()).expect("source")),
        started_at: v["startedAt"].as_str().map(str::to_owned),
        completed_at: v["completedAt"].as_str().map(str::to_owned),
        duration_ms: v["durationMs"].as_u64(),
        error: v["error"].as_str().map(str::to_owned),
    }
}

#[test]
fn a_new_session_matches_typescript() {
    let fx = common::fixture("session-conformance.json");
    let session = base_session(&fx);
    assert_eq!(serde_json::to_value(&session).unwrap(), fx["session"]);
}

#[test]
fn lifecycle_walks_match_typescript() {
    let fx = common::fixture("session-conformance.json");
    for case in fx["sessions"].as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        let mut session = base_session(&fx);
        let mut trail = Vec::new();
        let mut error = None;
        for step in case["steps"].as_array().unwrap() {
            match transition_session(
                session.clone(),
                status(step["to"].as_str().unwrap()),
                patch(step.get("patch")),
            ) {
                Ok(next) => {
                    trail.push(next.status.as_str().to_owned());
                    session = next;
                }
                Err(e) => {
                    error = Some(e.to_string());
                    break;
                }
            }
        }
        assert_eq!(json!(trail), case["trail"], "{name}: trail");
        assert_eq!(
            error.as_deref(),
            case.get("error").and_then(Value::as_str),
            "{name}: error"
        );
        assert_eq!(
            serde_json::to_value(&session).unwrap(),
            case["final"],
            "{name}: final"
        );
    }
}

#[test]
fn the_whole_transition_table_matches_typescript() {
    let fx = common::fixture("session-conformance.json");
    let want: BTreeMap<String, Vec<String>> =
        serde_json::from_value(fx["allowed"].clone()).expect("table");
    let got: BTreeMap<String, Vec<String>> = ReviewLifecycleStatus::ALL
        .into_iter()
        .map(|from| {
            let targets = ReviewLifecycleStatus::ALL
                .into_iter()
                .filter(|to| debuggatha_core::review_engine::can_transition(from, *to))
                .map(|to| to.as_str().to_owned())
                .collect();
            (from.as_str().to_owned(), targets)
        })
        .collect();
    assert_eq!(got, want);
}
