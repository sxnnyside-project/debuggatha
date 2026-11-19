//! The input-required form of MCP 2026-07-28: a server may not ask the client anything, so a call
//! that needs the client's model answers `input_required` with the prompts, and the client calls
//! again with its answers and the `requestState` it was given. Here that state names a review the
//! daemon keeps suspended (`review/resume`).

use std::collections::HashMap;
use std::hash::{BuildHasher, RandomState};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

pub(crate) const ERA: &str = "2026-07-28";
const VERSION: &str = "io.modelcontextprotocol/protocolVersion";
const CAPABILITIES: &str = "io.modelcontextprotocol/clientCapabilities";
/// The daemon drops a suspended review after two minutes; its state is forgotten a bit later.
const KEPT: Duration = Duration::from_secs(300);

/// The version a request of the stateless era names in its `_meta`, if it names one.
pub(crate) fn version(params: &Value) -> Option<&str> {
    params["_meta"][VERSION].as_str()
}

pub(crate) fn can_sample(params: &Value) -> bool {
    params["_meta"][CAPABILITIES].get("sampling").is_some()
}

/// The prompt a daemon asks of the client's model, as a `sampling/createMessage` request.
pub(crate) fn sampling(params: &Value) -> Value {
    json!({
        "messages": [{ "role": "user", "content": { "type": "text", "text": params["user"] } }],
        "systemPrompt": params["system"],
        "maxTokens": params["maxTokens"],
    })
}

/// The prompts of a suspended review, keyed by the ids the daemon gave them.
pub(crate) fn input_requests(inputs: &Value) -> Value {
    let requests: serde_json::Map<String, Value> = inputs
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|input| {
            let key = input["id"].as_str()?;
            Some((
                key.to_owned(),
                json!({ "method": "sampling/createMessage", "params": sampling(&input["params"]) }),
            ))
        })
        .collect();
    Value::Object(requests)
}

/// What the client answered, as the daemon takes it: the text and model, or a refusal.
pub(crate) fn responses(answers: &Value) -> Value {
    let taken: serde_json::Map<String, Value> = answers
        .as_object()
        .into_iter()
        .flatten()
        .map(|(key, answer)| {
            let content = &answer["content"];
            let taken = if content["type"] == "text" {
                json!({ "text": content["text"], "model": answer["model"] })
            } else {
                json!({ "declined": true })
            };
            (key.clone(), taken)
        })
        .collect();
    Value::Object(taken)
}

struct Suspended {
    at: usize,
    token: String,
    /// Which call it was (tool and arguments): a state does not carry over to another call.
    call: String,
    since: Instant,
}

/// The calls waiting for a response from a daemon, and the reviews suspended between two calls.
#[derive(Default)]
pub(crate) struct Pending {
    waiting: Mutex<HashMap<String, Sender<Value>>>,
    suspended: Mutex<HashMap<String, Suspended>>,
    keys: Mutex<u64>,
}

fn locked<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

impl Pending {
    /// Be told the response to daemon request `id`.
    pub(crate) fn await_response(&self, id: &str) -> Receiver<Value> {
        let (sender, receiver) = channel();
        locked(&self.waiting).insert(id.to_owned(), sender);
        receiver
    }

    /// A response from a daemon, to whoever waits for it.
    pub(crate) fn route(&self, response: &Value) {
        let sender = response["id"]
            .as_str()
            .and_then(|id| locked(&self.waiting).remove(id));
        if let Some(sender) = sender {
            drop(sender.send(response.clone()));
        }
    }

    /// An unguessable key for a review that waits: the client must not be able to name another's.
    /// `RandomState` is seeded by the operating system for each process; it is not a CSPRNG, and
    /// the key only reaches a loopback server that also checks the call it belongs to.
    pub(crate) fn remember(&self, at: usize, token: &str, call: &str) -> String {
        let mut count = locked(&self.keys);
        *count += 1;
        let hasher = RandomState::new();
        let key = format!(
            "{:016x}{:016x}",
            hasher.hash_one((*count, 1u8)),
            hasher.hash_one((*count, 2u8))
        );
        let mut suspended = locked(&self.suspended);
        suspended.retain(|_, s| s.since.elapsed() < KEPT);
        suspended.insert(
            key.clone(),
            Suspended {
                at,
                token: token.to_owned(),
                call: call.to_owned(),
                since: Instant::now(),
            },
        );
        key
    }

    /// The token of the review a state names, if it belongs to this call.
    pub(crate) fn recall(&self, key: &str, at: usize, call: &str) -> Option<String> {
        let suspended = locked(&self.suspended);
        let found = suspended.get(key)?;
        (found.at == at && found.call == call).then(|| found.token.clone())
    }

    pub(crate) fn forget(&self, key: &str) {
        locked(&self.suspended).remove(key);
    }
}
