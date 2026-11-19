//! The MCP server proper: one handler of JSON-RPC messages for any transport, over the
//! repositories it may work on. Each repository has its own daemon (`debuggatha-engine`), made the
//! first time a call names it; what a call may name is decided by the person who started the
//! server and by the roots the client shares, never by the call.

use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use debuggatha_engine::daemon::{Daemon, Emitter, Request};
use serde_json::{Value, json};

use crate::mcp::input::{self, ERA, Pending};
use crate::mcp::prompts::{prompt_text, prompts};
use crate::mcp::protocol::{INSTRUCTIONS, VERSIONS, rpc_error, rpc_result, tool_result};
use crate::mcp::resources::{RESOURCES, resources};
use crate::mcp::tools::{Call, output_schema, review_params, tools};

pub(crate) fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

pub(crate) type Out = Arc<Mutex<std::io::Stdout>>;

pub(crate) fn send(out: &Mutex<std::io::Stdout>, message: &Value) {
    let mut out = lock(out);
    // A client that closed its end is gone; there is no one to tell.
    drop(writeln!(out, "{message}"));
    drop(out.flush());
}

/// How a repository gets its daemon.
pub(crate) type Make<'a> = Box<dyn Fn(&Path) -> Result<Daemon, String> + Send + Sync + 'a>;

pub(crate) struct Repositories<'a> {
    make: Make<'a>,
    /// The handshake every daemon gets: who the client is and what it can do.
    hello: Mutex<Value>,
    /// The repository calls without `rootDir` work on.
    startup: PathBuf,
    /// Roots the person allowed at the start (and anything under them).
    allowed: Vec<PathBuf>,
    /// Roots the client shares (and anything under them).
    shared: Mutex<Vec<PathBuf>>,
    /// The first shared root replaces the startup one: the person did not name a repository.
    follows_client: bool,
    daemons: Mutex<Vec<(PathBuf, Arc<Daemon>)>>,
}

const UNREADABLE: &str = "The repository path does not exist or cannot be read.";

impl<'a> Repositories<'a> {
    pub(crate) fn new(
        make: Make<'a>,
        startup: PathBuf,
        allowed: Vec<PathBuf>,
        follows_client: bool,
    ) -> Self {
        let mut allowed = allowed;
        allowed.push(startup.clone());
        Self {
            make,
            hello: Mutex::new(json!({
                "client": { "name": "mcp", "version": "0" },
                "protocol": { "min": 1, "max": 1 },
                "capabilities": {},
            })),
            startup,
            allowed,
            shared: Mutex::new(Vec::new()),
            follows_client,
            daemons: Mutex::new(Vec::new()),
        }
    }

    fn default_root(&self) -> PathBuf {
        if self.follows_client
            && let Some(first) = lock(&self.shared).first()
        {
            return first.clone();
        }
        self.startup.clone()
    }

    /// The repository a call works on: the default, or the one it names if it may.
    fn resolve(&self, asked: Option<&str>) -> Result<PathBuf, String> {
        let Some(path) = asked.filter(|p| !p.is_empty()) else {
            return Ok(self.default_root());
        };
        if !Path::new(path).is_absolute() {
            return Err("rootDir must be an absolute path.".to_owned());
        }
        let wanted = Path::new(path)
            .canonicalize()
            .map_err(|_| UNREADABLE.to_owned())?;
        let permitted = self
            .allowed
            .iter()
            .chain(lock(&self.shared).iter())
            .any(|root| wanted.starts_with(root));
        if permitted {
            Ok(wanted)
        } else {
            Err("That repository is not one this server may work on: start the server with --allow-root, or share it as a root of the client.".to_owned())
        }
    }

    /// The daemon of the repository a call names, and its place among the daemons.
    fn daemon(&self, asked: Option<&str>) -> Result<(usize, Arc<Daemon>), String> {
        let root = self.resolve(asked)?;
        let mut daemons = lock(&self.daemons);
        if let Some(at) = daemons.iter().position(|(r, _)| *r == root) {
            return Ok((at, Arc::clone(&daemons[at].1)));
        }
        let daemon = Arc::new((self.make)(&root)?);
        let hello = Request {
            id: "m-0".to_owned(),
            method: "hello".to_owned(),
            params: lock(&self.hello).clone(),
        };
        let quiet: Emitter = Arc::new(|_| {});
        drop(daemon.handle(&hello, &quiet));
        daemons.push((root, Arc::clone(&daemon)));
        Ok((daemons.len() - 1, daemon))
    }

    fn nth(&self, at: usize) -> Option<Arc<Daemon>> {
        lock(&self.daemons).get(at).map(|(_, d)| Arc::clone(d))
    }

    pub(crate) fn cancel_all(&self) {
        for (_, daemon) in lock(&self.daemons).iter() {
            daemon.cancel_all();
        }
    }
}

/// `%41` is `A`: the way a path travels in a `file:` URI or a query.
fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while at < bytes.len() {
        let hex = |i: usize| bytes.get(i).and_then(|b| (*b as char).to_digit(16));
        match (bytes[at], hex(at + 1), hex(at + 2)) {
            (b'%', Some(high), Some(low)) => {
                out.push(u8::try_from(high * 16 + low).unwrap_or(b'?'));
                at += 3;
            }
            (byte, ..) => {
                out.push(byte);
                at += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// How long a call waits for its review to answer or to ask for the client's model.
const ANSWER_WITHIN: Duration = Duration::from_secs(600);

fn tool_error(id: &Value, message: &str) -> Value {
    rpc_result(
        id,
        &json!({ "isError": true, "content": [{ "type": "text", "text": message }] }),
    )
}

pub(crate) struct Server<'a> {
    repos: Repositories<'a>,
    /// Where messages for the client go; none over HTTP, which never asks the client anything.
    out: Option<Out>,
    initialized: Mutex<bool>,
    client_roots: Mutex<bool>,
    /// MCP request id (as JSON text) to the daemon and the request that serve it, for `cancelled`.
    serving: Mutex<HashMap<String, (usize, String)>>,
    /// Daemon request id to the progress token of the MCP call.
    tokens: Arc<Mutex<HashMap<String, Value>>>,
    /// Calls waiting for a daemon, and the reviews suspended between a call and its retry.
    pending: Arc<Pending>,
    next: AtomicU64,
}

impl<'a> Server<'a> {
    pub(crate) fn new(repos: Repositories<'a>, out: Option<Out>) -> Self {
        Self {
            repos,
            out,
            initialized: Mutex::new(false),
            client_roots: Mutex::new(false),
            serving: Mutex::new(HashMap::new()),
            tokens: Arc::new(Mutex::new(HashMap::new())),
            pending: Arc::new(Pending::default()),
            next: AtomicU64::new(0),
        }
    }

    fn stateless(&self) -> bool {
        self.out.is_none()
    }

    pub(crate) fn cancel_all(&self) {
        self.repos.cancel_all();
    }

    /// What the daemon number `at` sends: a request for the client's model, and progress.
    fn emitter(&self, at: usize) -> Emitter {
        let out = self.out.clone();
        let tokens = Arc::clone(&self.tokens);
        let pending = Arc::clone(&self.pending);
        Arc::new(move |message: Value| {
            if message["type"] == "response" {
                pending.route(&message);
                return;
            }
            let Some(out) = &out else { return };
            match (message["type"].as_str(), message["method"].as_str()) {
                (Some("request"), Some("model/complete")) => {
                    let mut params = input::sampling(&message["params"]);
                    params["includeContext"] = json!("none");
                    send(
                        out,
                        &json!({
                            // Daemons number their requests alike; the client hears which is whose.
                            "jsonrpc": "2.0", "id": format!("{at}:{}", message["id"].as_str().unwrap_or_default()),
                            "method": "sampling/createMessage", "params": params,
                        }),
                    );
                }
                (Some("event"), Some("progress")) => {
                    let params = &message["params"];
                    let token = params["request"]
                        .as_str()
                        .and_then(|r| lock(&tokens).get(r).cloned());
                    if let Some(token) = token {
                        send(
                            out,
                            &json!({
                                "jsonrpc": "2.0", "method": "notifications/progress",
                                "params": { "progressToken": token, "progress": params["done"],
                                            "total": params["total"], "message": params["message"] },
                            }),
                        );
                    }
                }
                _ => {}
            }
        })
    }

    fn initialize(&self, id: &Value, params: &Value) -> Value {
        let asked = params["protocolVersion"].as_str().unwrap_or_default();
        let version = VERSIONS
            .iter()
            .find(|v| **v == asked)
            .unwrap_or(&VERSIONS[0]);
        if !self.stateless() {
            *lock(&self.repos.hello) = json!({
                "client": { "name": params["clientInfo"]["name"], "version": params["clientInfo"]["version"] },
                "protocol": { "min": 1, "max": 1 },
                "capabilities": { "sampling": params["capabilities"].get("sampling").is_some(), "progress": true },
            });
            *lock(&self.client_roots) = params["capabilities"].get("roots").is_some();
        }
        *lock(&self.initialized) = true;
        rpc_result(
            id,
            &json!({
                "protocolVersion": version,
                "capabilities": {
                    "tools": { "listChanged": false },
                    "prompts": { "listChanged": false },
                    "resources": { "subscribe": false, "listChanged": false },
                },
                "serverInfo": { "name": "debuggatha", "title": "Debuggatha", "version": env!("CARGO_PKG_VERSION") },
                "instructions": INSTRUCTIONS,
            }),
        )
    }

    fn tools_list() -> Value {
        let all: Vec<Value> = tools()
            .iter()
            .map(|t| {
                let mut input = (t.input)();
                input["properties"]["rootDir"] = json!({
                    "type": "string",
                    "description": "Absolute path of the repository, when it is not the one this server was started in. It must be one the person allowed (--allow-root) or one the client shares as a root.",
                });
                json!({
                    "name": t.name, "title": t.title, "description": t.description,
                    "inputSchema": input,
                    "outputSchema": output_schema(t.name),
                    "annotations": {
                        "readOnlyHint": t.read_only, "destructiveHint": t.destructive,
                        "idempotentHint": t.read_only || t.name == "update_finding",
                        "openWorldHint": false,
                    },
                })
            })
            .collect();
        json!({ "tools": all })
    }

    fn call_tool(&self, id: &Value, params: &Value) -> Value {
        let name = params["name"].as_str().unwrap_or_default();
        let args = params
            .get("arguments")
            .cloned()
            .unwrap_or_else(|| json!({}));
        let Some(tool) = tools().into_iter().find(|t| t.name == name) else {
            return rpc_error(id, -32602, "Unknown tool.");
        };
        let (method, daemon_params) = match &tool.call {
            Call::Review(scope) => (
                "review/run",
                scope(&args).map(|scope| review_params(scope, &args)),
            ),
            Call::Method(method, map) => (*method, map(&args)),
        };
        let daemon_params = match daemon_params {
            Ok(p) => p,
            Err(message) => return tool_error(id, &message),
        };
        let (at, daemon) = match self.repos.daemon(args["rootDir"].as_str()) {
            Ok(found) => found,
            Err(message) => return tool_error(id, &message),
        };
        if method == "review/run"
            && daemon_params["semantic"] == json!(true)
            && input::version(params) == Some(ERA)
        {
            if !input::can_sample(params) {
                return json!({ "jsonrpc": "2.0", "id": id, "error": {
                    "code": -32021,
                    "message": "A review that reads the code with a model needs a client that can sample.",
                    "data": { "requiredCapabilities": { "sampling": {} } },
                } });
            }
            let call = format!("{name}:{args}");
            return self.call_asking(id, params, (at, daemon), daemon_params, &call);
        }
        let request_id = format!("m-{}", self.next.fetch_add(1, Ordering::Relaxed) + 1);
        lock(&self.serving).insert(id.to_string(), (at, request_id.clone()));
        if let Some(token) = params["_meta"].get("progressToken") {
            lock(&self.tokens).insert(request_id.clone(), token.clone());
        }
        let request = Request {
            id: request_id.clone(),
            method: method.to_owned(),
            params: daemon_params,
        };
        let response = daemon.handle(&request, &self.emitter(at));
        lock(&self.serving).remove(&id.to_string());
        lock(&self.tokens).remove(&request_id);
        match response {
            Some(response) => rpc_result(id, &tool_result(&response)),
            None => rpc_error(id, -32603, "The call did not answer."),
        }
    }

    /// A review whose model is the client's, for a client that cannot be asked: it answers with
    /// the prompts and the state of the suspended review, and the retry brings the answers.
    fn call_asking(
        &self,
        id: &Value,
        params: &Value,
        (at, daemon): (usize, Arc<Daemon>),
        mut daemon_params: Value,
        call: &str,
    ) -> Value {
        let request_id = format!("m-{}", self.next.fetch_add(1, Ordering::Relaxed) + 1);
        let waiting = self.pending.expect(&request_id);
        let emit = self.emitter(at);
        let state = params["requestState"].as_str();
        if let Some(key) = state {
            let Some(token) = self.pending.recall(key, at, call) else {
                return rpc_error(id, -32602, "The requestState does not belong to this call.");
            };
            let request = Request {
                id: request_id.clone(),
                method: "review/resume".to_owned(),
                params: json!({ "token": token, "responses": input::responses(&params["inputResponses"]) }),
            };
            if let Some(response) = daemon.handle(&request, &emit) {
                self.pending.route(&response);
            }
        } else {
            daemon_params["inputRequired"] = json!(true);
            let request = Request {
                id: request_id.clone(),
                method: "review/run".to_owned(),
                params: daemon_params,
            };
            let pending = Arc::clone(&self.pending);
            std::thread::spawn(move || {
                if let Some(response) = daemon.handle(&request, &emit) {
                    pending.route(&response);
                }
            });
        }
        let Ok(response) = waiting.recv_timeout(ANSWER_WITHIN) else {
            return tool_error(id, "The review did not answer.");
        };
        let result = &response["result"];
        if response["ok"] == json!(true) && result["status"] == "input_required" {
            let key = state.map_or_else(
                || {
                    self.pending
                        .remember(at, result["token"].as_str().unwrap_or_default(), call)
                },
                str::to_owned,
            );
            return rpc_result(
                id,
                &json!({
                    "resultType": "input_required",
                    "inputRequests": input::input_requests(&result["inputs"]),
                    "requestState": key,
                }),
            );
        }
        if let Some(key) = state {
            self.pending.forget(key);
        }
        rpc_result(id, &tool_result(&response))
    }

    pub(crate) fn handle(&self, message: &Value) -> Option<Value> {
        let mut response = self.dispatch(message)?;
        // The stateless era marks every result complete unless it says otherwise.
        if input::version(&message["params"]) == Some(ERA)
            && let Some(result) = response.get_mut("result").and_then(Value::as_object_mut)
        {
            result.entry("resultType").or_insert(json!("complete"));
        }
        Some(response)
    }

    fn discover(id: &Value) -> Value {
        rpc_result(
            id,
            &json!({
                "supportedVersions": [ERA, VERSIONS[0]],
                "capabilities": {
                    "tools": { "listChanged": false },
                    "prompts": { "listChanged": false },
                    "resources": { "subscribe": false, "listChanged": false },
                },
                "_meta": { "io.modelcontextprotocol/serverInfo": { "name": "debuggatha", "version": env!("CARGO_PKG_VERSION") } },
                "instructions": INSTRUCTIONS,
            }),
        )
    }

    fn dispatch(&self, message: &Value) -> Option<Value> {
        let id = message.get("id").cloned();
        let method = message["method"].as_str().unwrap_or_default();
        let params = message.get("params").cloned().unwrap_or_else(|| json!({}));
        let Some(id) = id else {
            match method {
                "notifications/cancelled" => self.cancel(&params["requestId"]),
                "notifications/initialized" | "notifications/roots/list_changed" => {
                    self.ask_for_roots();
                }
                _ => {}
            }
            return None;
        };
        let era = input::version(&params);
        if let Some(asked) = era
            && asked != ERA
        {
            return Some(json!({ "jsonrpc": "2.0", "id": id, "error": {
                "code": -32022, "message": "Unsupported protocol version",
                "data": { "supported": [ERA, VERSIONS[0]], "requested": asked },
            } }));
        }
        if !self.stateless()
            && era.is_none()
            && method != "initialize"
            && method != "ping"
            && !*lock(&self.initialized)
        {
            return Some(rpc_error(&id, -32002, "The server is not initialized."));
        }
        Some(match method {
            "initialize" => self.initialize(&id, &params),
            "ping" => rpc_result(&id, &json!({})),
            "server/discover" => Self::discover(&id),
            "tools/list" => rpc_result(&id, &Self::tools_list()),
            "tools/call" => self.call_tool(&id, &params),
            "prompts/list" => rpc_result(&id, &json!({ "prompts": prompts() })),
            "prompts/get" => {
                let name = params["name"].as_str().unwrap_or_default();
                match prompt_text(name, &params["arguments"]) {
                    Some(text) => rpc_result(
                        &id,
                        &json!({ "messages": [{ "role": "user", "content": { "type": "text", "text": text } }] }),
                    ),
                    None => rpc_error(&id, -32602, "Unknown prompt."),
                }
            }
            "resources/list" => rpc_result(&id, &json!({ "resources": resources() })),
            "resources/templates/list" => {
                rpc_result(&id, &json!({ "resourceTemplates": resource_templates() }))
            }
            "resources/read" => self.read_resource(&id, &params),
            _ => rpc_error(&id, -32601, "That method is not part of this server."),
        })
    }

    /// A resource is a read-only method of the daemon, shown as JSON. `?rootDir=` names another
    /// repository, under the same rules as a tool call.
    fn read_resource(&self, id: &Value, params: &Value) -> Value {
        let uri = params["uri"].as_str().unwrap_or_default();
        let (base, query) = uri.split_once('?').unwrap_or((uri, ""));
        let Some(spec) = RESOURCES.iter().find(|r| r.uri == base) else {
            return rpc_error(id, -32002, "Resource not found.");
        };
        let root = query
            .split('&')
            .find_map(|pair| pair.strip_prefix("rootDir="))
            .map(percent_decode);
        let (at, daemon) = match self.repos.daemon(root.as_deref()) {
            Ok(found) => found,
            Err(message) => return rpc_error(id, -32602, &message),
        };
        let request = Request {
            id: format!("m-{}", self.next.fetch_add(1, Ordering::Relaxed) + 1),
            method: spec.method.to_owned(),
            params: spec.params(),
        };
        match daemon.handle(&request, &self.emitter(at)) {
            Some(response) if response["ok"] == json!(true) => {
                let body = serde_json::to_string_pretty(&response["result"]).unwrap_or_default();
                rpc_result(
                    id,
                    &json!({ "contents": [{ "uri": uri, "mimeType": "application/json", "text": body }] }),
                )
            }
            Some(response) => rpc_error(
                id,
                -32603,
                response["error"]["message"]
                    .as_str()
                    .unwrap_or("The resource could not be read."),
            ),
            None => rpc_error(id, -32603, "The resource could not be read."),
        }
    }

    fn cancel(&self, request: &Value) {
        let found = lock(&self.serving).get(&request.to_string()).cloned();
        if let Some((at, target)) = found
            && let Some(daemon) = self.repos.nth(at)
        {
            daemon.event(
                &json!({ "v": 1, "type": "event", "method": "cancel", "params": { "target": target } })
                    .to_string(),
            );
        }
    }

    /// A client that shares roots is asked for them after it says it is ready, and again when it
    /// says they changed. Only over stdio: HTTP cannot ask.
    fn ask_for_roots(&self) {
        let Some(out) = &self.out else { return };
        if !*lock(&self.client_roots) {
            return;
        }
        let n = self.next.fetch_add(1, Ordering::Relaxed) + 1;
        send(
            out,
            &json!({ "jsonrpc": "2.0", "id": format!("roots-{n}"), "method": "roots/list" }),
        );
    }

    /// A response from the client: the roots it shares, or the answer to a
    /// `sampling/createMessage` a daemon asked for.
    pub(crate) fn answer(&self, message: &Value) {
        let id = message["id"].as_str().unwrap_or_default();
        if id.starts_with("roots-") {
            self.set_roots(&message["result"]["roots"]);
            return;
        }
        let Some((at, original)) = id.split_once(':') else {
            return;
        };
        let Some(daemon) = at.parse().ok().and_then(|at| self.repos.nth(at)) else {
            return;
        };
        let mut response = json!({ "v": 1, "type": "response", "id": original });
        if message.get("error").is_some() {
            response["ok"] = json!(false);
            response["error"] = json!({ "code": "declined", "message": "The client declined." });
        } else {
            let content = &message["result"]["content"];
            response["ok"] = json!(true);
            response["result"] = json!({
                "text": if content["type"] == "text" { content["text"].clone() } else { Value::Null },
                "model": message["result"]["model"],
            });
        }
        daemon.event(&response.to_string());
    }

    fn set_roots(&self, roots: &Value) {
        let paths: Vec<PathBuf> = roots
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|root| root["uri"].as_str()?.strip_prefix("file://"))
            .filter_map(|path| PathBuf::from(percent_decode(path)).canonicalize().ok())
            .collect();
        *lock(&self.repos.shared) = paths;
    }
}

fn resource_templates() -> Value {
    Value::Array(
        RESOURCES
            .iter()
            .map(|r| {
                json!({ "uriTemplate": format!("{}{{?rootDir}}", r.uri),
                        "name": format!("{}-for-root", r.name), "title": r.title,
                        "description": r.description, "mimeType": "application/json" })
            })
            .collect(),
    )
}
