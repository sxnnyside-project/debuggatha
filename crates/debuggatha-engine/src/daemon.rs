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

use crate::analyzers::{ADAPTERS, AnalyzerOptions, Mode, inventory};
use crate::control::{Cancel, Control};
use crate::daemon_host::{Asked, ClientModel, Closing, Session, Suspending, repository_stamp};
use crate::diff_review::DiffReviewError;
use crate::git::GitError;
use crate::pipeline::{
    PipelineError, ReviewInput, ReviewOutput, Scope, SemanticInput, create_baseline,
    execute_review_with,
};
use crate::semantic_layer::{LocalProvider, SemanticOptions, SemanticProvider};

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
    pub root: PathBuf,
    /// Analyzers enabled by name although they run project code or use the network.
    pub enabled_analyzers: Vec<String>,
    pub analyzer_search: Vec<PathBuf>,
    /// A model on this machine, when the person started the daemon with one.
    pub semantic: Option<LocalProvider>,
    /// Clients may use their own model for the semantic pass, if they offer it at the handshake.
    pub client_model: bool,
}

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
    pub id: String,
    pub method: String,
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
            "analyzers/list" => Ok(self.analyzers_list()),
            "memory/list" => self.memory_list(request),
            "memory/transition" => self.memory_transition(request),
            _ => Err(fail(
                "unknown_method",
                "That method is not part of protocol 1.",
            )),
        }
    }

    fn analyzer_options(&self, requested: &AnalyzerParams) -> AnalyzerOptions {
        AnalyzerOptions {
            mode: if requested.mode.as_deref() == Some("off") {
                Mode::Off
            } else {
                Mode::Auto
            },
            // Enabling is the daemon's, never the request's.
            enable: self.config.enabled_analyzers.clone(),
            disable: requested.disable.clone(),
            timeout_seconds: requested.timeout_seconds.filter(|s| *s > 0),
        }
    }

    fn scope(&self, scope: &ScopeParams) -> Result<Scope, Failure> {
        match scope.kind.as_str() {
            "workspace" => Ok(Scope::Workspace),
            "files" => scope
                .paths
                .iter()
                .map(|p| inside_root(&self.config.root, p))
                .collect::<Result<Vec<_>, _>>()
                .map(Scope::Files),
            "diff" => Ok(match &scope.diff {
                Some(text) => Scope::DiffText(text.clone()),
                None => Scope::Diff {
                    base: scope.base.clone(),
                },
            }),
            other => Err(fail(
                "invalid_request",
                format!("The scope \"{other}\" is not workspace, files or diff."),
            )),
        }
    }

    fn review_run(&self, request: &Request, emit: &Emitter) -> Answer {
        let cancel = Cancel::default();
        // A client that cannot be asked gets the second form: the review may stop and wait.
        // A stateless client says with each call (`inputRequired`) what it can answer.
        let suspends = request.params["semantic"] == json!(true)
            && self.config.client_model
            && (request.params["inputRequired"] == json!(true)
                || (!self.sampling.load(Ordering::SeqCst)
                    && self.input_required.load(Ordering::SeqCst)));
        let session = suspends.then(|| {
            let token = format!("r-{:x}", self.tokens.fetch_add(1, Ordering::Relaxed) + 1);
            let stamp = repository_stamp(&self.config.root);
            let session = Session::new(token.clone(), stamp, &request.id, cancel.clone());
            lock_suspended(&self.suspended).insert(token, Arc::clone(&session));
            session
        });
        let outcome = self.run_review(request, emit, cancel, session.as_deref());
        let Some(session) = session else {
            return Answer::Now(outcome);
        };
        lock_suspended(&self.suspended).retain(|_, s| !Arc::ptr_eq(s, &session));
        match session.closing() {
            Closing::Original => Answer::Now(outcome),
            Closing::To(id) => {
                emit(self.render(&id, outcome));
                Answer::Sent
            }
            Closing::Nobody => Answer::Sent,
        }
    }

    fn review_resume(&self, request: &Request) -> Answer {
        let outcome = (|| {
            let token = request.params["token"]
                .as_str()
                .ok_or_else(|| fail("invalid_request", "The parameter \"token\" is required."))?;
            let responses = request.params["responses"].as_object().ok_or_else(|| {
                fail(
                    "invalid_request",
                    "The parameter \"responses\" is required.",
                )
            })?;
            let session = lock_suspended(&self.suspended)
                .get(token)
                .cloned()
                .ok_or_else(|| fail("not_found", "No review is waiting under that token."))?;
            if repository_stamp(&self.config.root) != session.stamp {
                session.cancel.cancel();
                return Err(fail(
                    "conflict",
                    "The repository changed while the review waited; run it again.",
                ));
            }
            if session.deliver(&request.id, Value::Object(responses.clone())) {
                Ok(())
            } else {
                Err(fail("conflict", "That review is not waiting for answers."))
            }
        })();
        match outcome {
            Ok(()) => Answer::Sent,
            Err(failure) => Answer::Now(Err(failure)),
        }
    }

    fn run_review(
        &self,
        request: &Request,
        emit: &Emitter,
        cancel: Cancel,
        session: Option<&Session>,
    ) -> Outcome {
        let asked: ReviewParams = params(request)?;
        let scope = self.scope(&asked.scope)?;
        let persist = asked.persist.unwrap_or(true);
        let started = Instant::now();
        let sampled = (asked.semantic
            && self.config.client_model
            && self.sampling.load(Ordering::SeqCst)
            && session.is_none())
        .then(|| ClientModel::new(&self.asked, emit, cancel.clone()));
        let parked = session.map(|s| Suspending::new(s, emit, cancel.clone()));
        let local = (asked.semantic && sampled.is_none() && parked.is_none())
            .then_some(())
            .and(self.config.semantic.as_ref());
        let provider: Option<&dyn SemanticProvider> = sampled
            .as_ref()
            .map(|p| p as &dyn SemanticProvider)
            .or(parked.as_ref().map(|p| p as &dyn SemanticProvider))
            .or(local.map(|p| p as &dyn SemanticProvider));
        let input = ReviewInput {
            root: &self.config.root,
            scope,
            depth: ReviewDepth::Full,
            pack_ids: asked.pack_ids,
            policy_id: asked.policy_id,
            source_name: "debuggatha-daemon",
            persist,
            include_baselined: asked.include_baselined,
            analyzers: Some(self.analyzer_options(&asked.analyzers)),
            analyzer_search: self.config.analyzer_search.clone(),
            semantic: provider.map(|provider| SemanticInput {
                provider,
                options: SemanticOptions::default(),
            }),
        };
        lock_running(&self.running).insert(request.id.clone(), cancel.clone());
        let control = self.control(&request.id, cancel, emit);
        let turn = persist.then(|| lock(&self.writes));
        let output = execute_review_with(&input, &SystemEnv, &control);
        drop(turn);
        lock_running(&self.running).remove(&request.id);
        let output = output.map_err(|e| pipeline_failure(&e))?;
        Ok(review_result(
            &output,
            asked.limit.unwrap_or(DEFAULT_PAGE),
            started,
        ))
    }

    /// What lets a review be stopped, and, when the client wants it, hear its progress.
    fn control(&self, request: &str, cancel: Cancel, emit: &Emitter) -> Control {
        let control = Control::new(cancel);
        if !self.progress.load(Ordering::SeqCst) {
            return control;
        }
        let (emit, request) = (Arc::clone(emit), request.to_owned());
        let last = Mutex::new((None::<&'static str>, Instant::now()));
        control.listening(move |stage, done, total, message| {
            let mut last = last.lock().unwrap_or_else(PoisonError::into_inner);
            let changed = last.0 != Some(stage.name());
            if !changed && last.1.elapsed() < PROGRESS_EVERY {
                return;
            }
            *last = (Some(stage.name()), Instant::now());
            emit(json!({
                "v": PROTOCOL, "type": "event", "method": "progress",
                "params": { "request": request, "stage": stage.name(), "done": done,
                            "total": total, "message": message },
            }));
        })
    }

    fn review_cancel(&self, request: &Request) -> Outcome {
        let target = request.params["target"]
            .as_str()
            .ok_or_else(|| fail("invalid_request", "The parameter \"target\" is required."))?;
        self.cancel(target)
            .then(|| json!({ "target": target }))
            .ok_or_else(|| fail("not_found", "No review is running under that request id."))
    }

    fn cancel(&self, target: &str) -> bool {
        lock_running(&self.running).get(target).is_some_and(|c| {
            c.cancel();
            true
        })
    }

    /// The session is over: stop every review still running.
    pub fn cancel_all(&self) {
        for cancel in lock_running(&self.running).values() {
            cancel.cancel();
        }
        self.asked.drop_all();
    }

    /// An event or a response from the client. The only event is `cancel`; any other is ignored,
    /// as the tolerant reader of the protocol requires. A response answers a request of the
    /// daemon's (`model/complete`). `true` when the line was one of them, not a request.
    pub fn event(&self, line: &str) -> bool {
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            return false;
        };
        if value["type"] == "response" {
            self.asked.answer(&value);
            return true;
        }
        if value["type"] != "event" {
            return false;
        }
        if let (true, Some(target)) = (
            value["method"] == "cancel",
            value["params"]["target"].as_str(),
        ) {
            self.cancel(target);
        }
        true
    }

    fn findings_list(&self, request: &Request) -> Outcome {
        let asked: ListParams = params(request)?;
        let ledger = load_ledger(&self.config.root, &SystemEnv).map_err(|e| store_failure(&e))?;
        let filter = LedgerEntryFilter {
            status: (!asked.status.is_empty()).then_some(asked.status),
            severity: (!asked.severity.is_empty()).then_some(asked.severity),
            category: (!asked.category.is_empty()).then_some(asked.category),
            file: asked.file,
        };
        let entries = list_entries(&ledger, &filter);
        let start = match &asked.cursor {
            Some(cursor) => cursor
                .parse::<usize>()
                .map_err(|_| fail("invalid_request", "The cursor is not one this daemon gave."))?,
            None => 0,
        };
        let limit = asked.limit.unwrap_or(DEFAULT_PAGE).max(1);
        let page: Vec<_> = entries.iter().skip(start).take(limit).collect();
        let next = (start + page.len() < entries.len()).then(|| (start + page.len()).to_string());
        Ok(json!({ "entries": page, "nextCursor": next, "total": entries.len() }))
    }

    fn findings_get(&self, request: &Request) -> Outcome {
        let id = finding_id(request)?;
        let ledger = load_ledger(&self.config.root, &SystemEnv).map_err(|e| store_failure(&e))?;
        get_entry(&ledger, &id)
            .map(|entry| json!({ "entry": entry }))
            .ok_or_else(|| fail("not_found", "No finding has that id."))
    }

    /// Every lifecycle event of every finding, newest first.
    fn findings_history(&self) -> Outcome {
        let ledger = load_ledger(&self.config.root, &SystemEnv).map_err(|e| store_failure(&e))?;
        let mut events: Vec<Value> = ledger
            .entries
            .iter()
            .flat_map(|entry| {
                entry.history.iter().filter_map(|event| {
                    let mut event = serde_json::to_value(event).ok()?;
                    event["findingId"] = json!(entry.id);
                    event["fingerprint"] = serde_json::to_value(&entry.fingerprint).ok()?;
                    Some(event)
                })
            })
            .collect();
        events.sort_by(|a, b| b["timestamp"].as_str().cmp(&a["timestamp"].as_str()));
        Ok(json!({ "total": events.len(), "events": events }))
    }

    fn findings_update(&self, request: &Request) -> Outcome {
        let id = finding_id(request)?;
        let status: LifecycleStatus = serde_json::from_value(request.params["status"].clone())
            .map_err(|_| {
                fail(
                    "invalid_request",
                    "The parameter \"status\" is not a lifecycle status.",
                )
            })?;
        let comment = request.params["comment"].as_str().map(str::to_owned);
        let _turn = lock(&self.writes);
        let ledger = load_ledger(&self.config.root, &SystemEnv).map_err(|e| store_failure(&e))?;
        let updated = update_finding_status(
            &ledger,
            &id,
            status,
            TransitionOrigin::Manual {
                actor: "daemon".to_owned(),
            },
            comment,
            &SystemEnv,
        )
        .map_err(|e| match e {
            LedgerError::EntryNotFound(_) => fail("not_found", "No finding has that id."),
            illegal @ LedgerError::IllegalTransition { .. } => {
                fail("conflict", illegal.to_string())
            }
        })?;
        save_ledger(&updated).map_err(|e| store_failure(&e))?;
        Ok(json!({ "entry": get_entry(&updated, &id) }))
    }

    /// Everything needed to act on one finding: where it is, why it is a problem, how to fix it,
    /// and what to do if the code is right and the rule is wrong.
    fn findings_explain(&self, request: &Request) -> Outcome {
        let id = finding_id(request)?;
        let ledger = load_ledger(&self.config.root, &SystemEnv).map_err(|e| store_failure(&e))?;
        let entry =
            get_entry(&ledger, &id).ok_or_else(|| fail("not_found", "No finding has that id."))?;
        let entry = serde_json::to_value(entry).map_err(|e| internal(&e))?;
        let finding = &entry["latestFinding"];
        let file = entry["fingerprint"]["file"].as_str().unwrap_or_default();
        let rule = entry["fingerprint"]["ruleId"].as_str();
        let guidance = rule.and_then(crate::detect::guidance_for);
        let recommendation = &finding["recommendations"][0];
        let excerpt = finding["evidence"]
            .as_array()
            .and_then(|all| all.iter().find(|e| e["kind"] == "code"))
            .map(|e| e["excerpt"].clone());
        let history: Vec<Value> = entry["history"]
            .as_array()
            .map(|events| {
                events
                    .iter()
                    .map(|e| json!({ "timestamp": e["timestamp"], "state": e["newState"], "comment": e["comment"] }))
                    .collect()
            })
            .unwrap_or_default();
        Ok(json!({
            "finding": {
                "ledgerId": entry["id"], "status": entry["status"],
                "severity": finding["severity"], "confidence": finding["confidence"],
                "category": finding["category"], "title": finding["title"], "file": file,
                "line": finding["locations"][0]["lines"]["start"],
                "column": finding["locations"][0]["columns"]["start"],
                "excerpt": excerpt,
            },
            "rule": { "id": rule, "pack": finding["originatingPack"]["id"] },
            "why": guidance.map_or_else(|| finding["explanation"].clone(), |g| json!(g.why)),
            "fix": recommendation["summary"].as_str()
                .unwrap_or("No specific fix is recorded for this rule."),
            "example": recommendation["example"],
            "edit": recommendation["edit"],
            "ifYouAcceptIt": {
                "inlineComment": crate::syntax::inline_suppression_comment(
                    file, rule.unwrap_or("<rule>"), "<why this is acceptable>"),
                "durable": format!(
                    "findings/suppress with this id and a reason accepts every {} finding in {file}.",
                    rule.unwrap_or("such")),
            },
            "history": history,
        }))
    }

    /// Accepts a finding with a reason: every finding of its rule in its file stops being
    /// reported, the reason goes to the repository's memory, and the ledger entry is dismissed.
    fn findings_suppress(&self, request: &Request) -> Outcome {
        let id = finding_id(request)?;
        let reason = request.params["reason"]
            .as_str()
            .map(str::trim)
            .filter(|r| r.chars().count() >= 8)
            .ok_or_else(|| {
                fail(
                    "invalid_request",
                    "A reason of at least 8 characters is required.",
                )
            })?;
        let actor = "daemon";
        let env = &SystemEnv;
        let _turn = lock(&self.writes);
        let ledger = load_ledger(&self.config.root, env).map_err(|e| store_failure(&e))?;
        let entry =
            get_entry(&ledger, &id).ok_or_else(|| fail("not_found", "No finding has that id."))?;
        let Some(rule) = entry.fingerprint.rule_id.clone() else {
            return Err(fail(
                "conflict",
                "This finding has no rule behind it, so there is nothing to accept by rule.",
            ));
        };
        let file = entry.fingerprint.file.clone();
        let item = create_memory_item(
            NewMemoryItem {
                confidence: MemoryConfidence::Documented,
                rationale: reason.to_owned(),
                evidence: vec![MemoryEvidence {
                    file: file.clone(),
                    detail: format!("Accepted: {}", entry.latest_finding.title),
                }],
                detail: MemoryDetail::Suppression {
                    fingerprint_file: file.clone(),
                    fingerprint_rule_id: Some(rule.clone()),
                    fingerprint_category: entry.fingerprint.category.clone(),
                },
                origin: MemoryOrigin::User {
                    actor: actor.to_owned(),
                },
            },
            env,
        );
        let item_id = item.id.clone();
        let session = format!("daemon-{item_id}");
        let store = load_memory_store(&self.config.root, env).map_err(|e| store_failure(&e))?;
        let store = add_memory_item(&store, item, env);
        let store = suggest_memory(&store, &item_id, &session, env)
            .and_then(|s| confirm_memory(&s, &item_id, actor, Some(reason.to_owned()), env))
            .and_then(|s| activate_memory(&s, &item_id, actor, env))
            .map_err(|e| internal(&e))?;
        save_memory_store(&store).map_err(|e| store_failure(&e))?;
        if can_transition_finding(entry.status, LifecycleStatus::Dismissed) {
            let updated = update_finding_status(
                &ledger,
                &id,
                LifecycleStatus::Dismissed,
                TransitionOrigin::Manual {
                    actor: actor.to_owned(),
                },
                Some(reason.to_owned()),
                env,
            )
            .map_err(|e| internal(&e))?;
            save_ledger(&updated).map_err(|e| store_failure(&e))?;
        }
        Ok(json!({
            "memoryItemId": item_id, "file": file, "ruleId": rule, "reason": reason,
            "effect": format!("Every {rule} finding in {file} stops being reported."),
        }))
    }

    fn baseline_create(&self, request: &Request) -> Outcome {
        let asked: ReviewParams = serde_json::from_value(json!({
            "scope": { "kind": "workspace" },
            "packIds": request.params.get("packIds").cloned().unwrap_or_else(|| json!([])),
            "analyzers": request.params.get("analyzers").cloned().unwrap_or_else(|| json!({})),
        }))
        .map_err(|e| {
            fail(
                "invalid_request",
                format!("The parameters are not valid: {e}."),
            )
        })?;
        let _turn = lock(&self.writes);
        let (baseline, runs) = create_baseline(
            &ReviewInput {
                root: &self.config.root,
                scope: Scope::Workspace,
                depth: ReviewDepth::Full,
                pack_ids: asked.pack_ids,
                policy_id: None,
                source_name: "debuggatha-daemon",
                persist: false,
                include_baselined: true,
                analyzers: Some(self.analyzer_options(&asked.analyzers)),
                analyzer_search: self.config.analyzer_search.clone(),
                semantic: None,
            },
            &SystemEnv,
        )
        .map_err(|e| pipeline_failure(&e))?;
        let findings: u32 = baseline.entries.iter().map(|e| e.count).sum();
        Ok(json!({ "findings": findings, "analyzers": runs }))
    }

    fn repository_context(&self) -> Outcome {
        let context = build_repository_context(&self.config.root, &SystemEnv, None)
            .map_err(|e| pipeline_failure(&PipelineError::Scan(e)))?;
        Ok(json!({ "context": context }))
    }

    fn repository_summary(&self) -> Outcome {
        let context = build_repository_context(&self.config.root, &SystemEnv, None)
            .map_err(|e| pipeline_failure(&PipelineError::Scan(e)))?;
        Ok(json!({ "summary": summarize_capabilities(&context.capabilities) }))
    }

    fn analyzers_list(&self) -> Value {
        let options = AnalyzerOptions {
            enable: self.config.enabled_analyzers.clone(),
            ..AnalyzerOptions::default()
        };
        let entries = inventory(
            &self.config.root,
            &options,
            &self.config.analyzer_search,
            ADAPTERS,
        );
        let list: Vec<Value> = entries
            .iter()
            .map(|e| {
                json!({
                    "id": e.id, "name": e.name, "license": e.license, "trust": e.trust,
                    "applies": e.applies, "installed": e.installed, "version": e.version,
                    "willRun": e.will_run, "reason": e.reason,
                })
            })
            .collect();
        json!({ "analyzers": list })
    }

    fn memory_list(&self, request: &Request) -> Outcome {
        let status = request.params["status"].as_str();
        let store =
            load_memory_store(&self.config.root, &SystemEnv).map_err(|e| store_failure(&e))?;
        let items: Vec<_> = store
            .items
            .iter()
            .filter(|i| status.is_none_or(|s| i.status.as_str() == s))
            .collect();
        Ok(json!({ "items": items }))
    }

    fn memory_transition(&self, request: &Request) -> Outcome {
        let id = finding_id(request)?;
        let action = request.params["action"].as_str().unwrap_or_default();
        let comment = request.params["comment"].as_str().map(str::to_owned);
        let (env, actor) = (&SystemEnv, "daemon");
        let _turn = lock(&self.writes);
        let store = load_memory_store(&self.config.root, env).map_err(|e| store_failure(&e))?;
        let moved = match action {
            "suggest" => suggest_memory(&store, &id, actor, env),
            "confirm" => confirm_memory(&store, &id, actor, comment, env),
            "activate" => activate_memory(&store, &id, actor, env),
            "reject" => reject_memory(&store, &id, actor, comment, env),
            "deprecate" => deprecate_memory(&store, &id, actor, comment, env),
            "archive" => archive_memory(&store, &id, actor, comment, env),
            "remove" => remove_memory_item(&store, &id, env),
            _ => {
                return Err(fail(
                    "invalid_request",
                    "The action is not one of suggest, confirm, activate, reject, deprecate, archive or remove.",
                ));
            }
        }
        .map_err(|e| match e {
            MemoryError::NotFound(_) => fail("not_found", "No memory item has that id."),
            MemoryError::Transition(t) => fail("conflict", t.to_string()),
        })?;
        save_memory_store(&moved).map_err(|e| store_failure(&e))?;
        Ok(json!({ "item": moved.items.iter().find(|i| i.id == id) }))
    }
}

/// `review-job-result`: counts and records, plus the first page of findings by severity, and what
/// the review could not check. An empty page is never "clean" unless `coverage.complete` says so.
fn review_result(output: &ReviewOutput, limit: usize, started: Instant) -> Value {
    let mut findings: Vec<&Finding> = output.result.findings.iter().collect();
    findings.sort_by_key(|f| f.severity);
    let truncated = findings.len() > limit;
    findings.truncate(limit);
    json!({
        "findings": output.result.findings.len(),
        "analyzers": output.analyzers,
        "semantic": output.semantic,
        "changes": {
            "introduced": output.changes.introduced.len(),
            "fixed": output.changes.fixed.len(),
            "reopened": output.changes.reopened.len(),
        },
        "durationMs": u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
        "coverage": output.coverage,
        "findingsPage": { "findings": findings, "truncated": truncated },
        "policyId": output.policy.id,
        "baselined": output.baselined,
    })
}
