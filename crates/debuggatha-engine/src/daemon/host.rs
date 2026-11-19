//! The client's own model as a semantic provider: the daemon asks the client to complete a prompt
//! (`model/complete`, `docs/PROTOCOL.md` section 7) and waits for its answer. The daemon sends the
//! code to no model itself in this mode, and the client may decline.
//!
//! A client that cannot receive requests from the daemon gets the second form: the review that
//! needs a prompt answered is suspended and the response to the request that is waiting says
//! `input_required`; `review/resume` brings the answer and the review goes on.

use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use crate::control::Cancel;
use crate::daemon::{Emitter, PROTOCOL};
use crate::semantic_layer::{CompletionRequest, SemanticProvider};

/// How long a client may take to answer one prompt.
const ANSWER_WITHIN: Duration = Duration::from_secs(120);
/// How often a waiting call looks at the cancellation token.
const LOOK_EVERY: Duration = Duration::from_millis(100);

/// The requests the daemon has sent the client and has not seen answered, by request id.
#[derive(Default)]
pub(crate) struct Asked {
    senders: Mutex<HashMap<String, Sender<Value>>>,
    next: AtomicU64,
}

impl Asked {
    fn register(&self) -> (String, mpsc::Receiver<Value>) {
        let id = format!("s-{}", self.next.fetch_add(1, Ordering::Relaxed) + 1);
        let (tx, rx) = mpsc::channel();
        self.senders().insert(id.clone(), tx);
        (id, rx)
    }

    fn forget(&self, id: &str) {
        self.senders().remove(id);
    }

    fn senders(&self) -> std::sync::MutexGuard<'_, HashMap<String, Sender<Value>>> {
        self.senders.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// A response from the client. `true` when it answers a request of the daemon's.
    pub(crate) fn answer(&self, response: &Value) -> bool {
        let Some(id) = response["id"].as_str() else {
            return false;
        };
        self.senders()
            .remove(id)
            .is_some_and(|tx| tx.send(response.clone()).is_ok())
    }

    /// The session is over: nothing is waited for any more.
    pub(crate) fn drop_all(&self) {
        self.senders().clear();
    }
}

pub(crate) struct ClientModel<'a> {
    asked: &'a Asked,
    emit: &'a Emitter,
    cancel: Cancel,
    model: Mutex<Option<String>>,
}

impl<'a> ClientModel<'a> {
    pub(crate) fn new(asked: &'a Asked, emit: &'a Emitter, cancel: Cancel) -> Self {
        Self {
            asked,
            emit,
            cancel,
            model: Mutex::new(None),
        }
    }
}

impl SemanticProvider for ClientModel<'_> {
    fn name(&self) -> &'static str {
        "host"
    }

    fn model(&self) -> Option<String> {
        self.model
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    fn complete(&self, request: &CompletionRequest) -> Result<String, String> {
        let (id, answers) = self.asked.register();
        (self.emit)(json!({
            "v": PROTOCOL, "type": "request", "id": id, "method": "model/complete",
            "params": {
                "purpose": "semantic-review",
                "system": request.system,
                "user": request.user,
                "maxTokens": request.max_tokens,
            },
        }));
        let started = Instant::now();
        let response = loop {
            if self.cancel.is_cancelled() {
                self.asked.forget(&id);
                return Err("The review was cancelled.".to_owned());
            }
            match answers.recv_timeout(LOOK_EVERY) {
                Ok(response) => break response,
                Err(RecvTimeoutError::Timeout) if started.elapsed() < ANSWER_WITHIN => {}
                Err(_) => {
                    self.asked.forget(&id);
                    return Err("The client's model did not answer in time.".to_owned());
                }
            }
        };
        if response["ok"] != json!(true) {
            return Err(if response["error"]["code"] == "declined" {
                "The client declined to answer.".to_owned()
            } else {
                "The client's model could not answer.".to_owned()
            });
        }
        text_of(&response["result"], &self.model)
    }
}

/// The text of an answer to `model/complete`, and the model that gave it when it says so.
fn text_of(answer: &Value, model: &Mutex<Option<String>>) -> Result<String, String> {
    if answer["declined"] == json!(true) {
        return Err("The client declined to answer.".to_owned());
    }
    if let Some(name) = answer["model"].as_str() {
        *model.lock().unwrap_or_else(PoisonError::into_inner) = Some(name.to_owned());
    }
    answer["text"]
        .as_str()
        .filter(|text| !text.trim().is_empty())
        .map(str::to_owned)
        .ok_or_else(|| "The client's model answered with nothing.".to_owned())
}

/// How long a suspended review waits for `review/resume` before it is dropped.
const RESUME_WITHIN: Duration = Duration::from_secs(120);

struct Wait {
    /// The request that the next response goes to.
    reply_to: String,
    /// Is a request waiting for its response? False while the review waits for the client.
    pending: bool,
    answers: Option<Value>,
    /// Has the review answered a request with `input_required` at least once?
    suspended: bool,
}

/// A review that may stop to ask the client for a model answer, and the token that names it.
pub(crate) struct Session {
    token: String,
    /// Stops the review; what a resume that finds the repository changed does.
    pub(crate) cancel: Cancel,
    /// The state of the repository when the review began (`repository_stamp`).
    pub(crate) stamp: u64,
    wait: Mutex<Wait>,
    wake: Condvar,
    inputs: AtomicU64,
}

/// Where the end of a review answers.
pub(crate) enum Closing {
    /// The review never stopped: answer the request it began with.
    Original,
    /// It stopped and the client is waiting on this request.
    To(String),
    /// It stopped and no request waits (it was cancelled or expired): nothing to answer.
    Nobody,
}

impl Session {
    pub(crate) fn new(token: String, stamp: u64, request: &str, cancel: Cancel) -> Arc<Self> {
        Arc::new(Self {
            token,
            cancel,
            stamp,
            wait: Mutex::new(Wait {
                reply_to: request.to_owned(),
                pending: true,
                answers: None,
                suspended: false,
            }),
            wake: Condvar::new(),
            inputs: AtomicU64::new(0),
        })
    }

    fn wait(&self) -> MutexGuard<'_, Wait> {
        self.wait.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The client's answers to what the review asked. `false` when none were asked for.
    pub(crate) fn deliver(&self, request: &str, answers: Value) -> bool {
        let mut wait = self.wait();
        if wait.pending {
            return false;
        }
        request.clone_into(&mut wait.reply_to);
        wait.pending = true;
        wait.answers = Some(answers);
        self.wake.notify_all();
        true
    }

    pub(crate) fn closing(&self) -> Closing {
        let mut wait = self.wait();
        match (wait.suspended, wait.pending) {
            (false, _) => Closing::Original,
            (true, true) => {
                wait.pending = false;
                Closing::To(wait.reply_to.clone())
            }
            (true, false) => Closing::Nobody,
        }
    }
}

/// The provider of a review that suspends: each prompt goes to the client in an `input_required`
/// response, and the review waits for `review/resume`.
pub(crate) struct Suspending<'a> {
    session: &'a Session,
    emit: &'a Emitter,
    cancel: Cancel,
    model: Mutex<Option<String>>,
}

impl<'a> Suspending<'a> {
    pub(crate) fn new(session: &'a Session, emit: &'a Emitter, cancel: Cancel) -> Self {
        Self {
            session,
            emit,
            cancel,
            model: Mutex::new(None),
        }
    }
}

impl SemanticProvider for Suspending<'_> {
    fn name(&self) -> &'static str {
        "host"
    }

    fn model(&self) -> Option<String> {
        self.model
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    fn complete(&self, request: &CompletionRequest) -> Result<String, String> {
        if self.cancel.is_cancelled() {
            return Err("The review was cancelled.".to_owned());
        }
        let input = format!(
            "i-{}",
            self.session.inputs.fetch_add(1, Ordering::Relaxed) + 1
        );
        let mut wait = self.session.wait();
        wait.pending = false;
        wait.suspended = true;
        wait.answers = None;
        (self.emit)(json!({
            "v": PROTOCOL, "type": "response", "id": wait.reply_to, "ok": true,
            "result": {
                "status": "input_required",
                "token": self.session.token,
                "inputs": [{
                    "id": input,
                    "kind": "model/complete",
                    "params": {
                        "purpose": "semantic-review",
                        "system": request.system,
                        "user": request.user,
                        "maxTokens": request.max_tokens,
                    },
                }],
            },
        }));
        let started = Instant::now();
        let answers = loop {
            if let Some(answers) = wait.answers.take() {
                break answers;
            }
            if self.cancel.is_cancelled() {
                return Err("The review was cancelled.".to_owned());
            }
            if started.elapsed() >= RESUME_WITHIN {
                // A review that lost its client must not write the ledger when it is released.
                self.cancel.cancel();
                return Err("The review waited too long for the client.".to_owned());
            }
            wait = self
                .session
                .wake
                .wait_timeout(wait, LOOK_EVERY)
                .unwrap_or_else(PoisonError::into_inner)
                .0;
        };
        drop(wait);
        let answer = &answers[input.as_str()];
        if answer.is_null() {
            return Err("The client did not answer the prompt.".to_owned());
        }
        text_of(answer, &self.model)
    }
}

/// A number that changes when the files of the repository or the ledger do: what a resumed review
/// is checked against, so it never concludes on code other than the code the client's model read.
pub(crate) fn repository_stamp(root: &Path) -> u64 {
    let mut hasher = DefaultHasher::new();
    let stat = |relative: &str, hasher: &mut DefaultHasher| {
        let meta = std::fs::metadata(root.join(relative)).ok();
        relative.hash(hasher);
        meta.as_ref().map(std::fs::Metadata::len).hash(hasher);
        meta.and_then(|m| m.modified().ok()).hash(hasher);
    };
    let mut files = crate::scan_scope::list_owned_files(root);
    files.sort();
    for file in &files {
        stat(file, &mut hasher);
    }
    stat(".debuggatha/ledger.json", &mut hasher);
    hasher.finish()
}
