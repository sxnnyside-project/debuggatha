//! `debuggatha mcp`, driven as a process over stdio: one JSON-RPC object per line.
#![allow(clippy::expect_used, clippy::unwrap_used)] // a failed expect is a failed test

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use serde_json::{Value, json};

struct Repo(PathBuf);

impl Repo {
    fn new(name: &str) -> Self {
        let dir =
            std::env::temp_dir().join(format!("debuggatha-mcp-{name}-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("package.json"),
            r#"{"name":"t","version":"1.0.0","dependencies":{"express":"4.0.0"}}"#,
        )
        .unwrap();
        std::fs::write(dir.join("a.js"), "const x = eval(input);\nconst y = 1;\n").unwrap();
        Self(dir)
    }
}

impl Drop for Repo {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).ok();
    }
}

struct Server {
    child: Child,
    stdin: Option<ChildStdin>,
    stdout: BufReader<ChildStdout>,
    next: u32,
    /// What arrived while a response was awaited, other than that response.
    others: Vec<Value>,
}

impl Server {
    fn start(repo: &Repo) -> Self {
        Self::launch(&["--root", repo.0.to_str().unwrap()], None)
    }

    /// With the given options, in `cwd` if one is given.
    fn launch(options: &[&str], cwd: Option<&std::path::Path>) -> Self {
        let mut command = Command::new(env!("CARGO_BIN_EXE_debuggatha"));
        command.arg("mcp").args(options);
        if let Some(cwd) = cwd {
            command.current_dir(cwd);
        }
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let stdin = child.stdin.take();
        let stdout = BufReader::new(child.stdout.take().unwrap());
        Self {
            child,
            stdin,
            stdout,
            next: 0,
            others: Vec::new(),
        }
    }

    fn send(&mut self, message: &Value) {
        let stdin = self.stdin.as_mut().unwrap();
        writeln!(stdin, "{message}").unwrap();
        stdin.flush().unwrap();
    }

    fn read(&mut self) -> Value {
        let mut line = String::new();
        self.stdout.read_line(&mut line).unwrap();
        serde_json::from_str(&line).unwrap_or_else(|_| panic!("not JSON: {line:?}"))
    }

    /// Sends a request and returns its response. A request from the server meanwhile is answered
    /// by `serve` (a sampling request), anything else is kept.
    fn call(&mut self, method: &str, params: &Value, serve: &dyn Fn(&Value) -> Value) -> Value {
        self.next += 1;
        let id = self.next;
        self.send(&json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }));
        loop {
            let message = self.read();
            if message["method"] == "sampling/createMessage" {
                let result = serve(&message);
                self.send(&json!({ "jsonrpc": "2.0", "id": message["id"], "result": result }));
                self.others.push(message);
            } else if message["id"] == id && message.get("method").is_none() {
                return message;
            } else {
                self.others.push(message);
            }
        }
    }

    fn request(&mut self, method: &str, params: &Value) -> Value {
        self.call(method, params, &|_| json!({}))
    }

    fn initialize(&mut self, capabilities: &Value) -> Value {
        let response = self.request(
            "initialize",
            &json!({ "protocolVersion": "2025-06-18", "capabilities": capabilities,
                     "clientInfo": { "name": "test", "version": "0" } }),
        );
        self.send(&json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }));
        response
    }

    fn finish(mut self) -> Option<i32> {
        drop(self.stdin.take());
        self.child.wait().unwrap().code()
    }
}

#[test]
fn nothing_but_ping_works_before_initialize_and_the_handshake_names_the_server() {
    let repo = Repo::new("init");
    let mut server = Server::start(&repo);
    assert_eq!(
        server.request("tools/list", &json!({}))["error"]["code"],
        -32002
    );
    assert_eq!(server.request("ping", &json!({}))["result"], json!({}));

    let hello = server.initialize(&json!({}));
    assert_eq!(hello["result"]["protocolVersion"], "2025-06-18");
    assert_eq!(hello["result"]["serverInfo"]["name"], "debuggatha");
    assert!(
        hello["result"]["instructions"]
            .as_str()
            .unwrap()
            .contains("review_changes")
    );

    let old = server.request(
        "initialize",
        &json!({ "protocolVersion": "1999-01-01", "capabilities": {}, "clientInfo": {} }),
    );
    assert_eq!(
        old["result"]["protocolVersion"], "2025-11-25",
        "the newest it speaks"
    );
    assert_eq!(server.request("nope", &json!({}))["error"]["code"], -32601);
    assert_eq!(server.finish(), Some(0), "end of input ends the server");
}

#[test]
fn the_tools_of_the_typescript_server_are_all_here_with_schemas() {
    let repo = Repo::new("tools");
    let mut server = Server::start(&repo);
    server.initialize(&json!({}));
    let listed = server.request("tools/list", &json!({}));
    let tools = listed["result"]["tools"].as_array().unwrap();
    let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
    for expected in [
        "review_changes",
        "review_diff",
        "review_files",
        "review_workspace",
        "explain_finding",
        "suppress_finding",
        "list_findings",
        "get_finding",
        "update_finding",
        "create_baseline",
        "list_analyzers",
        "repository_context",
        "repository_summary",
    ] {
        assert!(
            names.contains(&expected),
            "{expected} is missing: {names:?}"
        );
    }
    assert!(tools.iter().all(|t| t["inputSchema"]["type"] == "object"));
    let suppress = tools
        .iter()
        .find(|t| t["name"] == "suppress_finding")
        .unwrap();
    assert_eq!(suppress["annotations"]["destructiveHint"], true);
    let list = server.request("prompts/list", &json!({}));
    assert_eq!(list["result"]["prompts"].as_array().unwrap().len(), 3);
    let prompt = server.request(
        "prompts/get",
        &json!({ "name": "fix-findings", "arguments": { "severity": "high" } }),
    );
    assert!(
        prompt["result"]["messages"][0]["content"]["text"]
            .as_str()
            .unwrap()
            .contains("severity high")
    );
    assert_eq!(
        server.request("prompts/get", &json!({ "name": "x" }))["error"]["code"],
        -32602
    );
    server.finish();
}

#[test]
fn a_review_finds_explains_and_accepts_through_tools() {
    let repo = Repo::new("flow");
    let mut server = Server::start(&repo);
    server.initialize(&json!({}));
    let call = |server: &mut Server, name: &str, arguments: Value| {
        server.request(
            "tools/call",
            &json!({ "name": name, "arguments": arguments }),
        )
    };

    let review = call(&mut server, "review_files", json!({ "paths": ["a.js"] }));
    let result = &review["result"];
    assert_ne!(result["isError"], true, "{review}");
    assert_ne!(result["structuredContent"]["findings"], 0);
    assert!(
        result["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("findings")
    );

    let listed = call(
        &mut server,
        "list_findings",
        json!({ "status": ["open"], "severity": ["critical", "high", "medium", "low"] }),
    );
    let id = listed["result"]["structuredContent"]["entries"][0]["id"].clone();
    let explained = call(&mut server, "explain_finding", json!({ "findingId": id }));
    assert_eq!(
        explained["result"]["structuredContent"]["finding"]["file"],
        "a.js"
    );

    let short = call(
        &mut server,
        "suppress_finding",
        json!({ "findingId": id, "reason": "no" }),
    );
    assert_eq!(
        short["result"]["isError"], true,
        "a refusal is a tool error, not a protocol one"
    );
    let accepted = call(
        &mut server,
        "suppress_finding",
        json!({ "findingId": id, "reason": "the input is a constant here" }),
    );
    assert_ne!(accepted["result"]["isError"], true, "{accepted}");

    let missing = call(&mut server, "review_files", json!({}));
    assert_eq!(missing["result"]["isError"], true);
    let unknown = call(&mut server, "no_such_tool", json!({}));
    assert_eq!(unknown["error"]["code"], -32602);
    let outside = call(&mut server, "review_files", json!({ "paths": ["../x.js"] }));
    assert_eq!(
        outside["result"]["isError"], true,
        "a path outside the repository is refused"
    );
    assert!(
        !outside["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains(&*repo.0.to_string_lossy())
    );
    server.finish();
}

#[test]
fn a_client_that_can_sample_is_the_model_and_one_that_cannot_gets_none() {
    let issue = r#"{"issues":[{"line":2,"quote":"const y = 1;","title":"Odd","why":"Looks odd.","category":"reliability","severity":"low"}]}"#;
    let repo = Repo::new("sampling");
    let mut server = Server::start(&repo);
    server.initialize(&json!({ "sampling": {} }));
    let reply = |_: &Value| {
        json!({ "role": "assistant", "model": "the-clients-model",
                "content": { "type": "text", "text": issue } })
    };
    let review = server.call(
        "tools/call",
        &json!({ "name": "review_files", "arguments": { "paths": ["a.js"], "semantic": true, "persist": false },
                 "_meta": { "progressToken": "p-1" } }),
        &reply,
    );
    let semantic = &review["result"]["structuredContent"]["semantic"];
    assert_eq!(semantic["provider"], "host", "{review}");
    assert_eq!(semantic["model"], "the-clients-model");
    assert_eq!(semantic["detected"], 1);
    let asked = server
        .others
        .iter()
        .find(|m| m["method"] == "sampling/createMessage")
        .unwrap();
    assert!(
        asked["params"]["messages"][0]["content"]["text"]
            .as_str()
            .unwrap()
            .contains("const y = 1;")
    );
    assert!(
        server
            .others
            .iter()
            .any(|m| m["method"] == "notifications/progress"
                && m["params"]["progressToken"] == "p-1"),
        "progress goes to the token the call gave"
    );
    server.finish();

    let mut plain = Server::start(&repo);
    plain.initialize(&json!({}));
    let review = plain.request(
        "tools/call",
        &json!({ "name": "review_files", "arguments": { "paths": ["a.js"], "semantic": true, "persist": false } }),
    );
    assert_eq!(
        review["result"]["structuredContent"]["semantic"],
        Value::Null
    );
    assert!(
        plain.others.is_empty(),
        "nothing is sent to a client that cannot sample"
    );
    plain.finish();
}

#[test]
fn the_ledger_and_the_context_are_resources() {
    let repo = Repo::new("resources");
    let mut server = Server::start(&repo);
    let hello = server.initialize(&json!({}));
    assert_eq!(
        hello["result"]["capabilities"]["resources"]["subscribe"],
        false
    );

    let listed = server.request("resources/list", &json!({}));
    let uris: Vec<&str> = listed["result"]["resources"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["uri"].as_str().unwrap())
        .collect();
    assert_eq!(
        uris,
        [
            "debuggatha://repository/context",
            "debuggatha://repository/summary",
            "debuggatha://ledger",
            "debuggatha://ledger/history"
        ]
    );
    let templates = server.request("resources/templates/list", &json!({}));
    assert_eq!(
        templates["result"]["resourceTemplates"]
            .as_array()
            .unwrap()
            .len(),
        4
    );
    assert_eq!(
        templates["result"]["resourceTemplates"][2]["uriTemplate"],
        "debuggatha://ledger{?rootDir}"
    );

    let read = |server: &mut Server, uri: &str| {
        let response = server.request("resources/read", &json!({ "uri": uri }));
        assert_eq!(
            response["result"]["contents"][0]["mimeType"], "application/json",
            "{response}"
        );
        serde_json::from_str::<Value>(response["result"]["contents"][0]["text"].as_str().unwrap())
            .unwrap()
    };
    assert_eq!(read(&mut server, "debuggatha://ledger")["total"], 0);
    assert!(read(&mut server, "debuggatha://repository/context").is_object());

    server.request(
        "tools/call",
        &json!({ "name": "review_files", "arguments": { "paths": ["a.js"] } }),
    );
    let ledger = read(&mut server, "debuggatha://ledger");
    assert_ne!(ledger["total"], 0);
    let history = read(&mut server, "debuggatha://ledger/history");
    assert_ne!(history["total"], 0);
    assert!(history["events"][0]["findingId"].is_string());

    let unknown = server.request("resources/read", &json!({ "uri": "debuggatha://nope" }));
    assert_eq!(unknown["error"]["code"], -32002);
    server.finish();
}

fn review(server: &mut Server, root: Option<&Path>) -> Value {
    let mut arguments = json!({ "paths": ["a.js"], "persist": false });
    if let Some(root) = root {
        arguments["rootDir"] = json!(root);
    }
    server.request(
        "tools/call",
        &json!({ "name": "review_files", "arguments": arguments }),
    )
}

#[test]
fn a_call_may_name_another_repository_only_if_the_person_or_the_client_allowed_it() {
    let (home, other, stranger) = (Repo::new("home"), Repo::new("other"), Repo::new("stranger"));
    std::fs::write(
        other.0.join("a.js"),
        "const a = 1;\nconst b = eval(x);\nconst c = eval(y);\n",
    )
    .unwrap();

    let mut server = Server::launch(
        &[
            "--root",
            home.0.to_str().unwrap(),
            "--allow-root",
            other.0.to_str().unwrap(),
        ],
        None,
    );
    server.initialize(&json!({}));
    let here = review(&mut server, None);
    let there = review(&mut server, Some(&other.0));
    assert_eq!(here["result"]["structuredContent"]["findings"], 1);
    assert_eq!(
        there["result"]["structuredContent"]["findings"], 2,
        "{there}"
    );

    let refused = review(&mut server, Some(&stranger.0));
    assert_eq!(refused["result"]["isError"], true);
    let text = refused["result"]["content"][0]["text"].as_str().unwrap();
    assert!(text.contains("--allow-root"));
    assert!(
        !text.contains(&*stranger.0.to_string_lossy()),
        "no machine path in the message"
    );
    assert_eq!(
        review(&mut server, Some(Path::new("relative")))["result"]["isError"],
        true
    );
    assert_eq!(
        review(&mut server, Some(&stranger.0.join("nope")))["result"]["isError"],
        true
    );

    let by_resource = server.request(
        "resources/read",
        &json!({ "uri": format!("debuggatha://ledger?rootDir={}", other.0.to_str().unwrap().replace('/', "%2F")) }),
    );
    assert!(
        by_resource["result"]["contents"][0]["text"].is_string(),
        "{by_resource}"
    );
    let refused = server.request(
        "resources/read",
        &json!({ "uri": format!("debuggatha://ledger?rootDir={}", stranger.0.to_str().unwrap()) }),
    );
    assert!(refused["error"].is_object());
    server.finish();
}

#[test]
fn the_roots_a_client_shares_are_allowed_and_the_first_is_the_default_when_none_was_named() {
    let (cwd, shared) = (Repo::new("cwd"), Repo::new("shared"));
    std::fs::write(
        shared.0.join("a.js"),
        "const a = eval(x);\nconst b = eval(y);\nconst c = eval(z);\n",
    )
    .unwrap();
    let shared_dir = shared.0.canonicalize().unwrap();

    let mut server = Server::launch(&[], Some(&cwd.0));
    server.initialize(&json!({ "roots": { "listChanged": true } }));
    // The server asks for the roots once the client says it is ready.
    let asked = loop {
        let message = server.read();
        if message["method"] == "roots/list" {
            break message;
        }
    };
    server.send(&json!({ "jsonrpc": "2.0", "id": asked["id"], "result": {
        "roots": [{ "uri": format!("file://{}", shared_dir.display()), "name": "shared" }] } }));

    let default = review(&mut server, None);
    assert_eq!(
        default["result"]["structuredContent"]["findings"], 3,
        "no repository was named, so the client's first root is the default: {default}"
    );
    let own = review(&mut server, Some(&cwd.0));
    assert_eq!(
        own["result"]["structuredContent"]["findings"], 1,
        "the directory it started in is allowed too: {own}"
    );
    server.finish();

    let mut named = Server::launch(&["--root", cwd.0.to_str().unwrap()], None);
    named.initialize(&json!({ "roots": {} }));
    let asked = loop {
        let message = named.read();
        if message["method"] == "roots/list" {
            break message;
        }
    };
    named.send(&json!({ "jsonrpc": "2.0", "id": asked["id"], "result": {
        "roots": [{ "uri": format!("file://{}", shared_dir.display()) }] } }));
    assert_eq!(
        review(&mut named, None)["result"]["structuredContent"]["findings"],
        1,
        "a repository the person named stays the default"
    );
    named.finish();
}
