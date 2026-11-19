//! `debuggatha mcp --http`, driven over a real socket with hand-written HTTP requests.
#![allow(clippy::expect_used, clippy::unwrap_used)] // a failed expect is a failed test

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};

use serde_json::{Value, json};

struct Repo(PathBuf);

impl Repo {
    fn new(name: &str) -> Self {
        let dir =
            std::env::temp_dir().join(format!("debuggatha-http-{name}-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("package.json"),
            r#"{"name":"t","version":"1.0.0","dependencies":{"express":"4.0.0"}}"#,
        )
        .unwrap();
        std::fs::write(dir.join("a.js"), "const x = eval(input);\n").unwrap();
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
    port: u16,
}

impl Server {
    fn start(repo: &Repo) -> Self {
        Self::start_with(repo, &[])
    }

    fn start_with(repo: &Repo, more: &[&str]) -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_debuggatha"))
            .args(["mcp", "--http", "--port", "0", "--root"])
            .arg(&repo.0)
            .args(more)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let mut line = String::new();
        BufReader::new(child.stderr.take().unwrap())
            .read_line(&mut line)
            .unwrap();
        let port = line
            .rsplit(':')
            .next()
            .unwrap()
            .split('/')
            .next()
            .unwrap()
            .parse()
            .unwrap();
        Self { child, port }
    }

    /// One request, one response: (status, headers, body).
    fn http(
        &self,
        method: &str,
        path: &str,
        headers: &[(&str, &str)],
        body: &str,
    ) -> (u16, String, String) {
        let mut stream = TcpStream::connect(("127.0.0.1", self.port)).unwrap();
        let host = format!("127.0.0.1:{}", self.port);
        let mut lines = vec![format!("{method} {path} HTTP/1.1")];
        if !headers.iter().any(|(n, _)| n.eq_ignore_ascii_case("host")) {
            lines.push(format!("Host: {host}"));
        }
        lines.extend(
            headers
                .iter()
                .map(|(name, value)| format!("{name}: {value}")),
        );
        lines.push(format!("Content-Length: {}", body.len()));
        lines.push("Connection: close".to_owned());
        let head = format!("{}\r\n\r\n{body}", lines.join("\r\n"));
        stream.write_all(head.as_bytes()).unwrap();
        let mut raw = String::new();
        stream.read_to_string(&mut raw).unwrap();
        let (head, body) = raw.split_once("\r\n\r\n").unwrap();
        let status = head.split_whitespace().nth(1).unwrap().parse().unwrap();
        (status, head.to_owned(), body.to_owned())
    }

    fn rpc(&self, message: &Value) -> Value {
        let (status, _, body) = self.http(
            "POST",
            "/mcp",
            &[("Content-Type", "application/json")],
            &message.to_string(),
        );
        assert_eq!(status, 200, "{body}");
        serde_json::from_str(&body).unwrap()
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.child.kill().ok();
        self.child.wait().ok();
    }
}

#[test]
fn each_post_is_answered_on_its_own_and_a_notification_is_accepted() {
    let repo = Repo::new("posts");
    let server = Server::start(&repo);
    let init = server.rpc(&json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize",
        "params": { "protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": { "name": "t", "version": "0" } } }));
    assert_eq!(init["result"]["serverInfo"]["name"], "debuggatha");

    // No session: a call needs no earlier initialize on the same connection.
    let listed = server.rpc(&json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" }));
    assert_eq!(listed["result"]["tools"].as_array().unwrap().len(), 13);
    let review = server.rpc(&json!({ "jsonrpc": "2.0", "id": 3, "method": "tools/call",
        "params": { "name": "review_files", "arguments": { "paths": ["a.js"] } } }));
    assert_ne!(review["result"]["structuredContent"]["findings"], 0);
    let ledger = server.rpc(
        &json!({ "jsonrpc": "2.0", "id": 4, "method": "resources/read",
        "params": { "uri": "debuggatha://ledger" } }),
    );
    assert!(
        ledger["result"]["contents"][0]["text"]
            .as_str()
            .unwrap()
            .contains("a.js")
    );

    let (status, _, body) = server.http(
        "POST",
        "/mcp",
        &[("Content-Type", "application/json")],
        r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
    );
    assert_eq!((status, body.as_str()), (202, ""));
    let batch = server.rpc(&json!([
        { "jsonrpc": "2.0", "id": 5, "method": "ping" },
        { "jsonrpc": "2.0", "method": "notifications/initialized" },
        { "jsonrpc": "2.0", "id": 6, "method": "prompts/list" },
    ]));
    assert_eq!(batch.as_array().unwrap().len(), 2);
}

#[test]
fn only_post_to_mcp_from_a_local_host_is_answered() {
    let repo = Repo::new("refusals");
    let server = Server::start(&repo);
    let json_type = [("Content-Type", "application/json")];
    let ping = r#"{"jsonrpc":"2.0","id":1,"method":"ping"}"#;

    let (status, head, _) = server.http("GET", "/mcp", &[], "");
    assert_eq!(status, 405);
    assert!(head.contains("Allow: POST"));
    assert_eq!(server.http("POST", "/elsewhere", &json_type, ping).0, 404);
    assert_eq!(
        server
            .http(
                "POST",
                "/mcp",
                &[("Host", "evil.example"), json_type[0]],
                ping
            )
            .0,
        403,
        "a rebinding page reaches the loopback port under a foreign Host"
    );
    assert_eq!(
        server
            .http(
                "POST",
                "/mcp",
                &[("Origin", "https://evil.example"), json_type[0]],
                ping
            )
            .0,
        403,
        "a web page of another origin"
    );
    assert_eq!(
        server
            .http(
                "POST",
                "/mcp",
                &[("Origin", "http://localhost:5173"), json_type[0]],
                ping
            )
            .0,
        200,
        "a page served from this machine"
    );
    let (status, _, body) = server.http("POST", "/mcp", &json_type, "{not json");
    assert_eq!(status, 400);
    assert!(body.contains("-32700"));
}

#[test]
fn an_address_that_is_not_loopback_is_refused_at_start() {
    let repo = Repo::new("bind");
    let output = Command::new(env!("CARGO_BIN_EXE_debuggatha"))
        .args(["mcp", "--http", "--host", "0.0.0.0", "--root"])
        .arg(&repo.0)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stderr).contains("loopback"));
}

#[test]
fn a_call_over_http_may_name_only_a_repository_the_person_allowed() {
    let (home, other, stranger) = (
        Repo::new("h-home"),
        Repo::new("h-other"),
        Repo::new("h-stranger"),
    );
    std::fs::write(
        other.0.join("a.js"),
        "const a = eval(x);\nconst b = eval(y);\n",
    )
    .unwrap();
    let server = Server::start_with(&home, &["--allow-root", other.0.to_str().unwrap()]);
    let call = |root: &std::path::Path| {
        server.rpc(
            &json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": {
            "name": "review_files",
            "arguments": { "paths": ["a.js"], "persist": false, "rootDir": root } } }),
        )
    };
    assert_eq!(call(&other.0)["result"]["structuredContent"]["findings"], 2);
    assert_eq!(call(&home.0)["result"]["structuredContent"]["findings"], 1);
    assert_eq!(call(&stranger.0)["result"]["isError"], true);
}

fn era(capabilities: &Value) -> Value {
    json!({
        "io.modelcontextprotocol/protocolVersion": "2026-07-28",
        "io.modelcontextprotocol/clientInfo": { "name": "t", "version": "0" },
        "io.modelcontextprotocol/clientCapabilities": capabilities,
    })
}

#[test]
fn a_review_that_needs_the_clients_model_asks_for_it_in_a_result_and_continues_on_the_retry() {
    let issue = r#"{"issues":[{"line":2,"quote":"const y = 1;","title":"Odd","why":"Looks odd.","category":"reliability","severity":"low"}]}"#;
    let repo = Repo::new("input-required");
    std::fs::write(
        repo.0.join("a.js"),
        "const x = eval(input);\nconst y = 1;\n",
    )
    .unwrap();
    let server = Server::start(&repo);
    let call = |id: u32, extra: Value, capabilities: &Value| {
        let mut params = json!({ "name": "review_files",
            "arguments": { "paths": ["a.js"], "semantic": true, "persist": false },
            "_meta": era(capabilities) });
        for (key, value) in extra.as_object().unwrap() {
            params[key] = value.clone();
        }
        server.rpc(&json!({ "jsonrpc": "2.0", "id": id, "method": "tools/call", "params": params }))
    };

    let discovered = server.rpc(
        &json!({ "jsonrpc": "2.0", "id": 0, "method": "server/discover",
        "params": { "_meta": era(&json!({})) } }),
    );
    assert_eq!(discovered["result"]["supportedVersions"][0], "2026-07-28");
    assert_eq!(discovered["result"]["resultType"], "complete");
    let other = server.rpc(&json!({ "jsonrpc": "2.0", "id": 0, "method": "tools/list",
        "params": { "_meta": { "io.modelcontextprotocol/protocolVersion": "1900-01-01" } } }));
    assert_eq!(other["error"]["code"], -32022, "{other}");
    assert_eq!(other["error"]["data"]["requested"], "1900-01-01");

    let missing = call(1, json!({}), &json!({}));
    assert_eq!(missing["error"]["code"], -32021, "{missing}");
    assert!(missing["error"]["data"]["requiredCapabilities"]["sampling"].is_object());

    let sampling = json!({ "sampling": {} });
    let first = call(2, json!({}), &sampling);
    assert_eq!(first["result"]["resultType"], "input_required", "{first}");
    let requests = first["result"]["inputRequests"].as_object().unwrap();
    assert_eq!(requests.len(), 1, "{first}");
    let (key, request) = requests.iter().next().unwrap();
    assert_eq!(request["method"], "sampling/createMessage");
    assert!(
        request["params"]["messages"][0]["content"]["text"]
            .as_str()
            .unwrap()
            .contains("const y = 1;")
    );
    let state = first["result"]["requestState"].as_str().unwrap().to_owned();

    let forged = call(
        3,
        json!({ "requestState": "0000", "inputResponses": {} }),
        &sampling,
    );
    assert_eq!(forged["error"]["code"], -32602, "{forged}");

    let answered = call(
        4,
        json!({ "requestState": state, "inputResponses": { key.as_str(): {
            "role": "assistant", "model": "the-clients-model",
            "content": { "type": "text", "text": issue } } } }),
        &sampling,
    );
    assert_eq!(answered["result"]["resultType"], "complete", "{answered}");
    let semantic = &answered["result"]["structuredContent"]["semantic"];
    assert_eq!(semantic["provider"], "host", "{answered}");
    assert_eq!(semantic["model"], "the-clients-model");
    assert_eq!(semantic["detected"], 1);

    let spent = call(
        5,
        json!({ "requestState": state, "inputResponses": {} }),
        &sampling,
    );
    assert_eq!(
        spent["error"]["code"], -32602,
        "a spent state is refused: {spent}"
    );

    // A client of the earlier version gets no model over HTTP, and no prompt either.
    let plain = server.rpc(&json!({ "jsonrpc": "2.0", "id": 6, "method": "tools/call",
        "params": { "name": "review_files",
            "arguments": { "paths": ["a.js"], "semantic": true, "persist": false } } }));
    assert_eq!(
        plain["result"]["structuredContent"]["semantic"],
        Value::Null,
        "{plain}"
    );
}
