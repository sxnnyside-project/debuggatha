//! The engine behind Protocol v1 (`docs/PROTOCOL.md`): requests in, responses out, one repository
//! root fixed at start. The transport (NDJSON on stdio) belongs to the caller; this module takes a
//! parsed [`Request`] and returns the response, so it is the same engine for any client.
//!
//! The trust rules of the protocol live here, not in the clients: a request can narrow what runs,
//! never enlarge it.

use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use debuggatha_core::env::SystemEnv;
use debuggatha_core::findings_ledger::{
    LedgerEntryFilter, LedgerError, LifecycleStatus, TransitionOrigin, can_transition_finding,
    get_entry, list_entries, load_ledger, save_ledger, update_finding_status,
};
use debuggatha_core::repository_intelligence::{build_repository_context, summarize_capabilities};
use debuggatha_core::repository_memory::{
    MemoryConfidence, MemoryDetail, MemoryError, MemoryEvidence, MemoryOrigin, NewMemoryItem,
    activate_memory, add_memory_item, archive_memory, confirm_memory, create_memory_item,
    deprecate_memory, load_memory_store, reject_memory, remove_memory_item, save_memory_store,
    suggest_memory,
};
use debuggatha_core::review_engine::{Finding, ReviewDepth, Severity};
use debuggatha_core::stored::StoreError;
use serde::Deserialize;
use serde_json::{Value, json};

use self::host::{Asked, ClientModel, Closing, Session, Suspending, repository_stamp};
use crate::analyzers::{ADAPTERS, AnalyzerOptions, Mode, inventory};
use crate::control::{Cancel, Control};
use crate::diff::review::DiffReviewError;
use crate::git::GitError;
use crate::pipeline::{
    PipelineError, ReviewInput, ReviewOutput, Scope, SemanticInput, assemble_policy,
    create_baseline, execute_review_with,
};
use crate::scan_scope::is_ignored_path;
use crate::semantic_layer::{LocalProvider, SemanticOptions, SemanticProvider};

mod findings;
mod host;
mod memory;
mod repository;
mod review;

/// The one protocol major this engine speaks.
pub const PROTOCOL: u32 = 1;
/// A line longer than this is a protocol error (`PROTOCOL.md`, section 1).
pub const MAX_LINE_BYTES: usize = 16 * 1024 * 1024;
const DEFAULT_PAGE: usize = 100;
/// Progress is a few messages a second, not one per rule.
const PROGRESS_EVERY: Duration = Duration::from_millis(250);

/// Where the daemon sends what is not a response: its events.
pub type Emitter = Arc<dyn Fn(Value) + Send + Sync>;

/// What the person who started the daemon allows. A client cannot enlarge it.
pub struct Config {
    /// The repository the daemon works on, fixed at start.
    pub root: PathBuf,
    /// Analyzers enabled by name although they run project code or use the network.
    pub enabled_analyzers: Vec<String>,
    /// Where analyzer executables are looked for after the repository's own.
    pub analyzer_search: Vec<PathBuf>,
    /// A model on this machine, when the person started the daemon with one.
    pub semantic: Option<LocalProvider>,
    /// Clients may use their own model for the semantic pass, if they offer it at the handshake.
    pub client_model: bool,
}

/// The engine behind Protocol v1: it answers requests for one repository root.
pub struct Daemon {
    config: Config,
    initialized: AtomicBool,
    /// The client asked for `progress` events at the handshake.
    progress: AtomicBool,
    /// The client announced that it can complete a prompt (`capabilities.sampling`).
    sampling: AtomicBool,
    /// The client announced that it cannot receive requests but can resume a review
    /// (`capabilities.inputRequired`).
    input_required: AtomicBool,
    /// What the daemon asked the client and has not had answered.
    asked: Asked,
    /// Reviews waiting for `review/resume`, by token.
    suspended: Mutex<HashMap<String, Arc<Session>>>,
    tokens: AtomicU64,
    errors: AtomicU64,
    /// Reviews that can still be stopped, by the id of their request.
    running: Mutex<HashMap<String, Cancel>>,
    /// Methods that write the ledger, baseline or memory take turns (`PROTOCOL.md`, section 6).
    writes: Mutex<()>,
}

/// A request that passed the envelope checks.
#[derive(Debug)]
pub struct Request {
    /// The request's id, echoed in the response.
    pub id: String,
    /// The method name, such as `review/run`.
    pub method: String,
    /// The method's parameters.
    pub params: Value,
}

/// What a method answers: a result, or a coded error.
type Outcome = Result<Value, Failure>;

/// What a review answers: now, or later through the emitter.
enum Answer {
    Now(Outcome),
    Sent,
}
/// Boxed: an error is rare and large, and a `Result` is not.
type Failure = Box<Fail>;

struct Fail {
    code: &'static str,
    message: String,
    /// The raw cause, logged to stderr with the `errorId`; never sent.
    detail: Option<String>,
    data: Option<Value>,
}

fn fail(code: &'static str, message: impl Into<String>) -> Failure {
    Box::new(Fail {
        code,
        message: message.into(),
        detail: None,
        data: None,
    })
}

fn internal(error: &(dyn std::error::Error + 'static)) -> Failure {
    let client = crate::client_error::classify_error(error);
    Box::new(Fail {
        code: "internal",
        message: client.client_message,
        detail: Some(client.log_detail),
        data: None,
    })
}

/// The finding a request named is not in the ledger; the ids come from a review or a listing.
fn unknown_finding() -> Failure {
    fail(
        "not_found",
        "No finding has that id. Use an id from a review result or from the findings list.",
    )
}

fn store_failure(error: &StoreError) -> Failure {
    internal(error)
}

fn pipeline_failure(error: &PipelineError) -> Failure {
    match error {
        PipelineError::Diff(DiffReviewError::Git(GitError::UnsafeRef)) => {
            fail("invalid_ref", GitError::UnsafeRef.to_string())
        }
        PipelineError::Diff(DiffReviewError::Git(git)) => fail("unavailable", git.to_string()),
        PipelineError::Cancelled => fail("cancelled", "The review was cancelled."),
        PipelineError::Policy(policy) => fail("conflict", policy.to_string()),
        PipelineError::Request(request) => fail("invalid_request", request.to_string()),
        other => internal(other),
    }
}

fn lock_running(
    running: &Mutex<HashMap<String, Cancel>>,
) -> MutexGuard<'_, HashMap<String, Cancel>> {
    running.lock().unwrap_or_else(PoisonError::into_inner)
}

fn lock_suspended(
    suspended: &Mutex<HashMap<String, Arc<Session>>>,
) -> MutexGuard<'_, HashMap<String, Arc<Session>>> {
    suspended.lock().unwrap_or_else(PoisonError::into_inner)
}

fn lock(mutex: &Mutex<()>) -> MutexGuard<'_, ()> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ScopeParams {
    kind: String,
    #[serde(default)]
    paths: Vec<String>,
    base: Option<String>,
    diff: Option<String>,
    /// For `files`: leave out what the project does not own (dependencies, build output, its own
    /// ledger), as a client does that reviews on save and cannot know the rules of the scan scope.
    #[serde(default)]
    owned: bool,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct AnalyzerParams {
    mode: Option<String>,
    #[serde(default)]
    disable: Vec<String>,
    timeout_seconds: Option<u64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReviewParams {
    scope: ScopeParams,
    #[serde(default)]
    pack_ids: Vec<String>,
    policy_id: Option<String>,
    #[serde(default)]
    analyzers: AnalyzerParams,
    persist: Option<bool>,
    #[serde(default)]
    include_baselined: bool,
    #[serde(default)]
    semantic: bool,
    limit: Option<usize>,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct ListParams {
    #[serde(default)]
    status: Vec<LifecycleStatus>,
    #[serde(default)]
    severity: Vec<Severity>,
    #[serde(default)]
    category: Vec<String>,
    file: Option<String>,
    limit: Option<usize>,
    cursor: Option<String>,
}

fn params<T: for<'de> Deserialize<'de>>(request: &Request) -> Result<T, Failure> {
    serde_json::from_value(request.params.clone()).map_err(|e| {
        fail(
            "invalid_request",
            format!("The parameters are not valid: {e}."),
        )
    })
}

fn finding_id(request: &Request) -> Result<String, Failure> {
    request
        .params
        .get("id")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| fail("invalid_request", "The parameter \"id\" is required."))
}

/// A path of a request, as the repository-relative path a review wants, or a refusal.
fn inside_root(root: &Path, path: &str) -> Result<String, Failure> {
    let outside = || fail("path_outside_root", "A path leaves the repository.");
    let given = Path::new(path);
    let relative: PathBuf = if given.is_absolute() {
        let real = |p: &Path| p.canonicalize().unwrap_or_else(|_| p.to_path_buf());
        real(given)
            .strip_prefix(real(root))
            .map_err(|_| outside())?
            .to_path_buf()
    } else {
        given.to_path_buf()
    };
    let mut parts: Vec<String> = Vec::new();
    for component in relative.components() {
        match component {
            Component::Normal(name) => parts.push(name.to_string_lossy().into_owned()),
            Component::CurDir => {}
            Component::ParentDir => {
                parts.pop().ok_or_else(outside)?;
            }
            Component::RootDir | Component::Prefix(_) => return Err(outside()),
        }
    }
    if parts.is_empty() {
        return Err(outside());
    }
    Ok(parts.join("/"))
}

fn ok(id: &str, result: &Value) -> Value {
    json!({ "v": PROTOCOL, "type": "response", "id": id, "ok": true, "result": result })
}

impl Daemon {
    /// A daemon for `config`, waiting for the handshake.
    #[must_use]
    pub fn new(config: Config) -> Self {
        Self {
            config,
            initialized: AtomicBool::new(false),
            progress: AtomicBool::new(false),
            sampling: AtomicBool::new(false),
            input_required: AtomicBool::new(false),
            asked: Asked::default(),
            suspended: Mutex::new(HashMap::new()),
            tokens: AtomicU64::new(0),
            errors: AtomicU64::new(0),
            running: Mutex::new(HashMap::new()),
            writes: Mutex::new(()),
        }
    }

    /// Parses one line. An envelope that cannot be answered by id (no id) yields `Err(None)`; one
    /// that can be gets its error response.
    ///
    /// # Errors
    /// The response to send, if the line can be answered at all.
    pub fn parse(&self, line: &str) -> Result<Request, Option<Value>> {
        let value: Value = serde_json::from_str(line).map_err(|_| None::<Value>)?;
        let id = value.get("id").and_then(Value::as_str).map(str::to_owned);
        let reply =
            |code, message: &str| id.as_deref().map(|i| self.error(i, &fail(code, message)));
        if value.get("v").and_then(Value::as_u64) != Some(u64::from(PROTOCOL))
            || value.get("type").and_then(Value::as_str) != Some("request")
        {
            return Err(reply("invalid_request", "Expected a version 1 request."));
        }
        let Some(method) = value.get("method").and_then(Value::as_str) else {
            return Err(reply("invalid_request", "A request needs a method."));
        };
        let Some(id) = id else { return Err(None) };
        Ok(Request {
            id,
            method: method.to_owned(),
            params: value.get("params").cloned().unwrap_or_else(|| json!({})),
        })
    }

    /// The answer to a line over the limit, which ends the connection.
    #[must_use]
    pub fn oversized_line(&self) -> Value {
        let mut response = self.error(
            "",
            &fail("invalid_request", "A message is longer than 16 MiB."),
        );
        response["id"] = Value::Null;
        response
    }

    fn error(&self, id: &str, failure: &Fail) -> Value {
        let error_id = format!("e-{:04x}", self.errors.fetch_add(1, Ordering::Relaxed) + 1);
        if let Some(detail) = &failure.detail {
            // debuggatha-ignore-next-line no-debug-output -- stderr is the diagnostics channel; stdout carries the protocol
            eprintln!("debuggatha: {error_id}: {detail}");
        }
        let mut error = json!({
            "code": failure.code, "message": failure.message, "errorId": error_id,
        });
        if let Some(data) = &failure.data {
            error["data"] = data.clone();
        }
        json!({ "v": PROTOCOL, "type": "response", "id": id, "ok": false, "error": error })
    }

    /// The response to a request. `shutdown` is the caller's to act on after sending it. `None`
    /// when the response goes out through the emitter: a review that stopped to ask the client
    /// answers whichever request is waiting when it ends.
    #[must_use]
    pub fn handle(&self, request: &Request, emit: &Emitter) -> Option<Value> {
        let outcome = if request.method == "hello" {
            self.hello(request)
        } else if !self.initialized.load(Ordering::SeqCst) {
            Err(fail("not_initialized", "The first message must be hello."))
        } else {
            match request.method.as_str() {
                "review/run" => match self.review_run(request, emit) {
                    Answer::Now(outcome) => outcome,
                    Answer::Sent => return None,
                },
                "review/resume" => match self.review_resume(request) {
                    Answer::Now(outcome) => outcome,
                    Answer::Sent => return None,
                },
                _ => self.dispatch(request),
            }
        };
        Some(self.render(&request.id, outcome))
    }

    fn render(&self, id: &str, outcome: Outcome) -> Value {
        match outcome {
            Ok(result) => ok(id, &result),
            Err(failure) => self.error(id, &failure),
        }
    }

    /// Did the client ask to end the session?
    #[must_use]
    pub fn is_shutdown(request: &Request) -> bool {
        request.method == "shutdown"
    }

    /// Did the handshake fail, so the daemon must exit after answering?
    #[must_use]
    pub fn handshake_failed(&self, request: &Request, response: &Value) -> bool {
        request.method == "hello" && response["ok"] == json!(false)
    }

    fn hello(&self, request: &Request) -> Outcome {
        let range = &request.params["protocol"];
        let (min, max) = (
            range["min"].as_u64().unwrap_or(0),
            range["max"].as_u64().unwrap_or(0),
        );
        if !(min..=max).contains(&u64::from(PROTOCOL)) {
            let mut failure = fail("protocol_mismatch", "This daemon speaks protocol 1.");
            failure.data = Some(json!({ "supported": [PROTOCOL] }));
            return Err(failure);
        }
        self.initialized.store(true, Ordering::SeqCst);
        self.progress.store(
            request.params["capabilities"]["progress"] == json!(true),
            Ordering::SeqCst,
        );
        self.sampling.store(
            request.params["capabilities"]["sampling"] == json!(true),
            Ordering::SeqCst,
        );
        self.input_required.store(
            request.params["capabilities"]["inputRequired"] == json!(true),
            Ordering::SeqCst,
        );
        let analyzers: Vec<&str> = ADAPTERS.iter().map(|a| a.id).collect();
        Ok(json!({
            "server": { "name": "debuggatha", "version": crate::VERSION },
            "protocol": PROTOCOL,
            "root": ".",
            "capabilities": {
                "analyzers": analyzers,
                "enabledAnalyzers": self.config.enabled_analyzers,
                "semantic": self.semantic_names(),
            },
        }))
    }

    /// The models a review may ask to read code: the one the person started the daemon with, or
    /// the client's own when the person allowed it and the client offers it.
    fn semantic_names(&self) -> Vec<&str> {
        if self.client_model_offered() {
            vec!["host"]
        } else {
            self.config
                .semantic
                .iter()
                .map(SemanticProvider::name)
                .collect()
        }
    }

    fn client_model_offered(&self) -> bool {
        self.config.client_model
            && (self.sampling.load(Ordering::SeqCst) || self.input_required.load(Ordering::SeqCst))
    }

    fn dispatch(&self, request: &Request) -> Outcome {
        match request.method.as_str() {
            "shutdown" => Ok(json!({})),
            "review/cancel" => self.review_cancel(request),
            "findings/explain" => self.findings_explain(request),
            "findings/suppress" => self.findings_suppress(request),
            "findings/list" => self.findings_list(request),
            "findings/get" => self.findings_get(request),
            "findings/history" => self.findings_history(),
            "findings/update" => self.findings_update(request),
            "baseline/create" => self.baseline_create(request),
            "repository/context" => self.repository_context(),
            "repository/summary" => self.repository_summary(),
            "repository/policy" => self.repository_policy(request),
            "analyzers/list" => Ok(self.analyzers_list()),
            "memory/list" => self.memory_list(request),
            "memory/transition" => self.memory_transition(request),
            _ => Err(fail(
                "unknown_method",
                "That method is not part of protocol 1.",
            )),
        }
    }
}
