//! The daemon, driven as a process over Protocol v1: one JSON object per line on stdin and stdout.
#![allow(clippy::expect_used, clippy::unwrap_used)] // a failed expect is a failed test

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use serde_json::{Value, json};

struct Repo(PathBuf);

impl Repo {
    fn new(name: &str) -> Self {
        let dir =
            std::env::temp_dir().join(format!("debuggatha-srv-{name}-{}", std::process::id()));
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

struct Daemon {
    child: Child,
    stdin: Option<ChildStdin>,
    stdout: BufReader<ChildStdout>,
    next: u32,
    /// Events that arrived while a response was awaited.
    events: Vec<Value>,
}

impl Daemon {
    fn start(repo: &Repo, args: &[&str]) -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_debuggatha"))
            .args(["serve", "--root"])
            .arg(&repo.0)
            .args(args)
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
            events: Vec::new(),
        }
    }

    fn send_raw(&mut self, line: &str) {
        let stdin = self.stdin.as_mut().unwrap();
        writeln!(stdin, "{line}").unwrap();
        stdin.flush().unwrap();
    }

    fn read(&mut self) -> Value {
        let mut line = String::new();
        self.stdout.read_line(&mut line).unwrap();
        serde_json::from_str(&line).unwrap_or_else(|_| panic!("not JSON: {line:?}"))
    }

    #[allow(clippy::needless_pass_by_value)] // call sites build the value inline
    fn request(&mut self, method: &str, params: Value) -> Value {
        self.next += 1;
        let id = format!("c-{}", self.next);
        self.send_raw(
            &json!({ "v": 1, "type": "request", "id": id, "method": method, "params": params })
                .to_string(),
        );
        self.response_to(&id)
    }

    /// The response with this id, keeping the events that come before it.
    fn response_to(&mut self, id: &str) -> Value {
        loop {
            let message = self.read();
            if message["type"] == "event" {
                self.events.push(message);
            } else {
                assert_eq!(message["id"], id);
                return message;
            }
        }
    }

    fn hello(&mut self) -> Value {
        self.request(
            "hello",
            json!({ "client": { "name": "test", "version": "0" }, "protocol": { "min": 1, "max": 1 }, "capabilities": {} }),
        )
    }

    fn finish(mut self) -> Option<i32> {
        drop(self.stdin.take());
        self.child.wait().unwrap().code()
    }
}

const EVAL: &str = r#"{ "scope": { "kind": "workspace" } }"#;

#[test]
fn nothing_works_before_hello_and_the_handshake_reports_what_the_person_enabled() {
    let repo = Repo::new("hello");
    let mut daemon = Daemon::start(&repo, &["--analyzer", "eslint"]);

    let early = daemon.request("review/run", serde_json::from_str(EVAL).unwrap());
    assert_eq!(early["error"]["code"], "not_initialized");

    let hello = daemon.hello();
    assert_eq!(hello["ok"], true);
    assert_eq!(hello["result"]["protocol"], 1);
    assert_eq!(hello["result"]["server"]["name"], "debuggatha");
    assert_eq!(
        hello["result"]["root"], ".",
        "the root is never the machine's path"
    );
    assert_eq!(
        hello["result"]["capabilities"]["enabledAnalyzers"],
        json!(["eslint"])
    );
    assert_eq!(hello["result"]["capabilities"]["semantic"], json!([]));
    assert_eq!(daemon.finish(), Some(0), "end of input is a shutdown");
}

#[test]
fn a_protocol_no_one_speaks_ends_the_session() {
    let repo = Repo::new("mismatch");
    let mut daemon = Daemon::start(&repo, &[]);
    let answer = daemon.request(
        "hello",
        json!({ "client": { "name": "t", "version": "0" }, "protocol": { "min": 2, "max": 3 } }),
    );
    assert_eq!(answer["error"]["code"], "protocol_mismatch");
    assert_eq!(answer["error"]["data"]["supported"], json!([1]));
    assert_eq!(daemon.finish(), Some(0));
}

#[test]
fn a_review_reports_findings_coverage_and_what_changed_and_the_ledger_follows() {
    let repo = Repo::new("review");
    let mut daemon = Daemon::start(&repo, &[]);
    daemon.hello();

    let review = daemon.request("review/run", serde_json::from_str(EVAL).unwrap());
    assert_eq!(review["ok"], true, "{review}");
    let result = &review["result"];
    assert_eq!(result["findings"], 1);
    assert_eq!(result["changes"]["introduced"], 1);
    assert_eq!(
        result["findingsPage"]["findings"][0]["locations"][0]["file"],
        "a.js"
    );
    assert_eq!(result["findingsPage"]["truncated"], false);
    assert!(
        result["coverage"]["complete"].is_boolean(),
        "a review says what it could not check"
    );

    let list = daemon.request("findings/list", json!({}));
    let entries = list["result"]["entries"].as_array().unwrap();
    assert_eq!(entries.len(), 1);
    let id = entries[0]["id"].as_str().unwrap().to_owned();

    let got = daemon.request("findings/get", json!({ "id": id }));
    assert_eq!(got["result"]["entry"]["status"], "open");
    let resolved = daemon.request("findings/update", json!({ "id": id, "status": "resolved" }));
    assert_eq!(resolved["result"]["entry"]["status"], "resolved");
    let illegal = daemon.request(
        "findings/update",
        json!({ "id": id, "status": "acknowledged" }),
    );
    assert_eq!(illegal["error"]["code"], "conflict");
    let missing = daemon.request("findings/get", json!({ "id": "nope" }));
    assert_eq!(missing["error"]["code"], "not_found");

    let page = daemon.request(
        "review/run",
        json!({ "scope": { "kind": "workspace" }, "limit": 0, "persist": false }),
    );
    assert_eq!(
        page["result"]["findingsPage"]["truncated"], true,
        "the page was cut, and says so"
    );
    daemon.finish();
}

#[test]
fn a_request_can_narrow_but_never_enlarge_what_runs() {
    let repo = Repo::new("trust");
    let mut daemon = Daemon::start(&repo, &[]);
    daemon.hello();
    let listed = daemon.request("analyzers/list", json!({}));
    let eslint = listed["result"]["analyzers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["id"] == "eslint")
        .unwrap();
    assert_eq!(eslint["trust"], "runs-project-code");
    assert_eq!(eslint["willRun"], false);

    let review = daemon.request(
        "review/run",
        json!({ "scope": { "kind": "workspace" }, "persist": false, "analyzers": { "enable": ["eslint"] },
                "semantic": true }),
    );
    let eslint_run = review["result"]["analyzers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["id"] == "eslint");
    assert!(
        eslint_run.is_none_or(|r| r["status"] != "ran"),
        "a request cannot enable it: {eslint_run:?}"
    );
    assert_eq!(
        review["result"]["semantic"],
        Value::Null,
        "no model was configured"
    );
    daemon.finish();
}

#[test]
fn bad_input_gets_a_coded_error_and_never_a_path() {
    let repo = Repo::new("errors");
    let mut daemon = Daemon::start(&repo, &[]);
    daemon.hello();

    let unknown = daemon.request("nope/never", json!({}));
    assert_eq!(unknown["error"]["code"], "unknown_method");

    let outside = daemon.request(
        "review/run",
        json!({ "scope": { "kind": "files", "paths": ["../x.js"] } }),
    );
    assert_eq!(outside["error"]["code"], "path_outside_root");
    let absolute = daemon.request(
        "review/run",
        json!({ "scope": { "kind": "files", "paths": ["/etc/passwd"] } }),
    );
    assert_eq!(absolute["error"]["code"], "path_outside_root");

    let git = Command::new("git")
        .args(["init", "-q"])
        .current_dir(&repo.0)
        .status()
        .unwrap();
    assert!(git.success());
    let reference = daemon.request(
        "review/run",
        json!({ "scope": { "kind": "diff", "base": "--output=x" } }),
    );
    assert_eq!(reference["error"]["code"], "invalid_ref");

    let bad = daemon.request("review/run", json!({ "scope": { "kind": "nowhere" } }));
    assert_eq!(bad["error"]["code"], "invalid_request");

    std::fs::create_dir_all(repo.0.join(".debuggatha")).unwrap();
    std::fs::write(repo.0.join(".debuggatha/ledger.json"), "{ not json").unwrap();
    let corrupt = daemon.request("findings/list", json!({}));
    assert_eq!(corrupt["error"]["code"], "internal");
    assert!(
        corrupt["error"]["errorId"]
            .as_str()
            .unwrap()
            .starts_with("e-")
    );
    assert!(
        !corrupt.to_string().contains(repo.0.to_str().unwrap()),
        "no path in a response"
    );

    daemon.send_raw("this is not json");
    daemon.send_raw(r#"{"v":1,"type":"request","method":"hello"}"#);
    let after = daemon.request("repository/summary", json!({}));
    assert_eq!(after["ok"], true, "garbage did not stop the daemon");
    daemon.finish();
}

#[test]
fn requests_run_side_by_side_and_answer_by_id() {
    let repo = Repo::new("concurrent");
    let mut daemon = Daemon::start(&repo, &[]);
    daemon.hello();
    for id in ["c-10", "c-11", "c-12"] {
        let method = match id {
            "c-10" => "repository/context",
            "c-11" => "analyzers/list",
            _ => "repository/summary",
        };
        daemon.send_raw(
            &json!({ "v": 1, "type": "request", "id": id, "method": method, "params": {} })
                .to_string(),
        );
    }
    let mut seen = BTreeMap::new();
    for _ in 0..3 {
        let response = daemon.read();
        seen.insert(
            response["id"].as_str().unwrap().to_owned(),
            response["ok"].clone(),
        );
    }
    assert_eq!(seen.len(), 3);
    assert!(seen.values().all(|ok| *ok == json!(true)));
    daemon.finish();
}

#[test]
fn the_policy_a_review_would_assemble_is_a_read_method() {
    let repo = Repo::new("policy");
    let mut daemon = Daemon::start(&repo, &[]);
    daemon.hello();
    let policy = daemon.request("repository/policy", json!({ "packIds": [] }));
    assert_eq!(policy["ok"], true, "{policy}");
    assert!(policy["result"]["policy"]["id"].is_string());
    assert_ne!(
        policy["result"]["policy"]["packRefs"]
            .as_array()
            .unwrap()
            .len(),
        0
    );
    daemon.finish();
}

#[test]
fn a_files_review_for_a_client_that_saves_leaves_out_what_the_project_does_not_own() {
    let repo = Repo::new("owned");
    std::fs::create_dir_all(repo.0.join("node_modules/dep")).unwrap();
    std::fs::write(
        repo.0.join("node_modules/dep/index.js"),
        "const x = eval(input);\n",
    )
    .unwrap();
    let mut daemon = Daemon::start(&repo, &[]);
    daemon.hello();
    let review = |daemon: &mut Daemon, owned: bool| {
        daemon.request(
            "review/run",
            json!({ "scope": { "kind": "files", "paths": ["node_modules/dep/index.js"], "owned": owned },
                    "persist": false }),
        )
    };
    assert_eq!(review(&mut daemon, false)["result"]["findings"], 1);
    let owned = review(&mut daemon, true);
    assert_eq!(owned["error"]["code"], "nothing_to_review", "{owned}");
    daemon.finish();
}

#[test]
fn shutdown_answers_and_ends_the_session() {
    let repo = Repo::new("shutdown");
    let mut daemon = Daemon::start(&repo, &[]);
    daemon.hello();
    let answer = daemon.request("shutdown", json!({}));
    assert_eq!(answer["ok"], true);
    let status = daemon.child.wait().unwrap();
    assert_eq!(status.code(), Some(0));
}

#[test]
fn a_message_over_the_limit_is_refused_and_closes_the_connection() {
    let repo = Repo::new("huge");
    let mut daemon = Daemon::start(&repo, &[]);
    daemon.hello();
    let mut stdin = daemon.stdin.take().unwrap();
    let writer = std::thread::spawn(move || {
        let chunk = vec![b'x'; 1 << 20];
        for _ in 0..17 {
            if stdin.write_all(&chunk).is_err() {
                break;
            }
        }
    });
    let answer = daemon.read();
    assert_eq!(answer["error"]["code"], "invalid_request");
    assert_eq!(answer["id"], Value::Null);
    assert_eq!(daemon.child.wait().unwrap().code(), Some(0));
    writer.join().unwrap();
}

#[cfg(unix)]
mod control {
    use std::os::unix::fs::PermissionsExt;
    use std::time::{Duration, Instant};

    use super::*;

    /// A `gitleaks` that never finishes, and records its process id so a test can look for it.
    fn hanging_gitleaks(repo: &Repo) -> (PathBuf, PathBuf) {
        let bin = repo.0.with_extension("bin");
        std::fs::create_dir_all(&bin).unwrap();
        let pid = bin.join("pid");
        let script = format!(
            "#!/bin/sh\nif [ \"$1\" = \"version\" ] || [ \"$1\" = \"--version\" ]; then echo 9.9.9; exit 0; fi\necho $$ > \"{}\"\nexec sleep 60\n",
            pid.display()
        );
        let path = bin.join("gitleaks");
        std::fs::write(&path, script).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        (bin, pid)
    }

    fn start_with_path(repo: &Repo, bin: &Path) -> Daemon {
        let system = std::env::var("PATH").unwrap_or_default();
        let mut child = Command::new(env!("CARGO_BIN_EXE_debuggatha"))
            .args(["serve", "--root"])
            .arg(&repo.0)
            .env("PATH", format!("{}:{system}", bin.display()))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let stdin = child.stdin.take();
        let stdout = BufReader::new(child.stdout.take().unwrap());
        Daemon {
            child,
            stdin,
            stdout,
            next: 0,
            events: Vec::new(),
        }
    }

    fn alive(pid: &str) -> bool {
        Command::new("kill")
            .args(["-0", pid.trim()])
            .stderr(Stdio::null())
            .status()
            .unwrap()
            .success()
    }

    fn wait_for(path: &Path) -> String {
        let deadline = Instant::now() + Duration::from_secs(10);
        while Instant::now() < deadline {
            if let Some(text) = std::fs::read_to_string(path)
                .ok()
                .filter(|t| !t.trim().is_empty())
            {
                return text;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        panic!("the analyzer never started");
    }

    #[test]
    fn a_cancelled_review_stops_its_analyzer_writes_nothing_and_answers_cancelled() {
        let repo = Repo::new("cancel");
        let (bin, pid_file) = hanging_gitleaks(&repo);
        let mut daemon = start_with_path(&repo, &bin);
        daemon.hello();

        daemon.send_raw(
            &json!({ "v": 1, "type": "request", "id": "r-1", "method": "review/run",
            "params": { "scope": { "kind": "workspace" } } })
            .to_string(),
        );
        let pid = wait_for(&pid_file);
        assert!(alive(&pid), "the analyzer is running");

        let started = Instant::now();
        let cancel = daemon.request("review/cancel", json!({ "target": "r-1" }));
        assert_eq!(cancel["ok"], true, "{cancel}");
        let answer = daemon.response_to("r-1");
        assert_eq!(answer["error"]["code"], "cancelled");
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "it did not wait for the analyzer"
        );

        let deadline = Instant::now() + Duration::from_secs(5);
        while alive(&pid) && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(!alive(&pid), "the analyzer process was killed");
        assert!(
            !repo.0.join(".debuggatha/ledger.json").exists(),
            "nothing was recorded"
        );

        let again = daemon.request("review/cancel", json!({ "target": "r-1" }));
        assert_eq!(
            again["error"]["code"], "not_found",
            "it is no longer running"
        );
        daemon.finish();
        std::fs::remove_dir_all(bin).ok();
    }

    #[test]
    fn ending_the_session_stops_what_still_runs() {
        let repo = Repo::new("eof");
        let (bin, pid_file) = hanging_gitleaks(&repo);
        let mut daemon = start_with_path(&repo, &bin);
        daemon.hello();
        daemon.send_raw(
            &json!({ "v": 1, "type": "request", "id": "r-2", "method": "review/run",
            "params": { "scope": { "kind": "workspace" } } })
            .to_string(),
        );
        let pid = wait_for(&pid_file);
        let started = Instant::now();
        assert_eq!(daemon.finish(), Some(0));
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "it did not wait for the analyzer"
        );
        let deadline = Instant::now() + Duration::from_secs(5);
        while alive(&pid) && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(!alive(&pid), "the analyzer process was killed");
        std::fs::remove_dir_all(bin).ok();
    }

    #[test]
    fn the_cancel_event_does_the_same_without_a_reply_of_its_own() {
        let repo = Repo::new("cancel-event");
        let (bin, pid_file) = hanging_gitleaks(&repo);
        let mut daemon = start_with_path(&repo, &bin);
        daemon.hello();
        daemon.send_raw(
            &json!({ "v": 1, "type": "request", "id": "r-9", "method": "review/run",
            "params": { "scope": { "kind": "workspace" } } })
            .to_string(),
        );
        wait_for(&pid_file);
        daemon.send_raw(
            &json!({ "v": 1, "type": "event", "method": "cancel",
            "params": { "target": "r-9" } })
            .to_string(),
        );
        assert_eq!(daemon.response_to("r-9")["error"]["code"], "cancelled");
        daemon.finish();
        std::fs::remove_dir_all(bin).ok();
    }
}

#[test]
fn progress_is_sent_only_to_a_client_that_asked_and_follows_the_stages() {
    let repo = Repo::new("progress");
    let mut quiet = Daemon::start(&repo, &[]);
    quiet.hello();
    quiet.request(
        "review/run",
        json!({ "scope": { "kind": "workspace" }, "persist": false }),
    );
    assert!(quiet.events.is_empty(), "no progress without asking");
    quiet.finish();

    let mut daemon = Daemon::start(&repo, &[]);
    daemon.request(
        "hello",
        json!({ "client": { "name": "t", "version": "0" }, "protocol": { "min": 1, "max": 1 },
                "capabilities": { "progress": true } }),
    );
    let review = daemon.request(
        "review/run",
        json!({ "scope": { "kind": "workspace" }, "persist": false }),
    );
    let stages: Vec<&str> = daemon
        .events
        .iter()
        .filter(|e| e["method"] == "progress")
        .map(|e| e["params"]["stage"].as_str().unwrap())
        .collect();
    assert_eq!(stages.first(), Some(&"orient"), "{stages:?}");
    assert!(stages.contains(&"detect"));
    assert_eq!(stages.last(), Some(&"sync"));
    assert!(
        daemon
            .events
            .iter()
            .all(|e| e["params"]["request"] == review["id"])
    );
    assert!(stages.len() < 40, "throttled: {}", stages.len());
    daemon.finish();
}

mod client_model {
    use super::*;

    const SUSPICION: &str = r#"{"issues":[{"line":1,"quote":"const x = 1;","title":"Suspicious","why":"Looks odd.","category":"reliability","severity":"low"}]}"#;

    impl Daemon {
        fn hello_offering_a_model(&mut self) -> Value {
            self.request(
                "hello",
                json!({ "client": { "name": "t", "version": "0" }, "protocol": { "min": 1, "max": 1 },
                        "capabilities": { "sampling": true } }),
            )
        }

        /// Sends the request and answers every `model/complete` the daemon asks meanwhile with
        /// `reply`. Returns the response and what the daemon asked.
        #[allow(clippy::needless_pass_by_value)] // call sites build the value inline
        fn request_and_answer(
            &mut self,
            method: &str,
            params: Value,
            reply: impl Fn(&Value) -> Value,
        ) -> (Value, Vec<Value>) {
            self.next += 1;
            let id = format!("c-{}", self.next);
            self.send_raw(
                &json!({ "v": 1, "type": "request", "id": id, "method": method, "params": params })
                    .to_string(),
            );
            let mut asked = Vec::new();
            loop {
                let message = self.read();
                match message["type"].as_str() {
                    Some("event") => self.events.push(message),
                    Some("request") => {
                        assert_eq!(message["method"], "model/complete");
                        let answer = reply(&message);
                        self.send_raw(&answer_to(&message, &answer).to_string());
                        asked.push(message);
                    }
                    _ => {
                        assert_eq!(message["id"], id);
                        return (message, asked);
                    }
                }
            }
        }
    }

    fn answer_to(request: &Value, outcome: &Value) -> Value {
        let base = json!({ "v": 1, "type": "response", "id": request["id"] });
        let rest = if outcome["declined"] == true {
            json!({ "ok": false, "error": { "code": "declined", "message": "No." } })
        } else {
            json!({ "ok": true, "result": outcome })
        };
        let mut response = base;
        response
            .as_object_mut()
            .unwrap()
            .extend(rest.as_object().unwrap().clone());
        response
    }

    fn files_review() -> Value {
        json!({ "scope": { "kind": "files", "paths": ["a.js"] }, "persist": false, "semantic": true })
    }

    fn repo(name: &str) -> Repo {
        let repo = Repo::new(name);
        std::fs::write(repo.0.join("a.js"), "const x = 1;\n").unwrap();
        repo
    }

    #[test]
    fn the_client_model_is_offered_only_when_the_person_allowed_it_and_the_client_has_one() {
        let repo = repo("cm-hello");
        let mut allowed = Daemon::start(&repo, &["--semantic", "host"]);
        let hello = allowed.hello_offering_a_model();
        assert_eq!(hello["result"]["capabilities"]["semantic"], json!(["host"]));
        allowed.finish();

        let mut no_model = Daemon::start(&repo, &["--semantic", "host"]);
        assert_eq!(
            no_model.hello()["result"]["capabilities"]["semantic"],
            json!([]),
            "the client has no model to offer"
        );
        no_model.finish();

        let mut not_allowed = Daemon::start(&repo, &[]);
        assert_eq!(
            not_allowed.hello_offering_a_model()["result"]["capabilities"]["semantic"],
            json!([]),
            "a client cannot turn the semantic layer on"
        );
        let (review, asked) =
            not_allowed.request_and_answer("review/run", files_review(), |_| json!({}));
        assert!(asked.is_empty(), "nothing was sent to the client's model");
        assert_eq!(review["result"]["semantic"], Value::Null);
        not_allowed.finish();
    }

    #[test]
    fn the_daemon_asks_the_client_and_keeps_only_what_quotes_the_code() {
        let repo = repo("cm-ask");
        let mut daemon = Daemon::start(&repo, &["--semantic", "host"]);
        daemon.hello_offering_a_model();
        let (review, asked) = daemon.request_and_answer(
            "review/run",
            files_review(),
            |_| json!({ "text": SUSPICION, "model": "the-clients-model" }),
        );
        assert_eq!(review["ok"], true, "{review}");
        assert_ne!(asked.len(), 0);
        assert!(asked[0]["id"].as_str().unwrap().starts_with("s-"));
        assert_eq!(asked[0]["params"]["purpose"], "semantic-review");
        assert!(
            asked[0]["params"]["user"]
                .as_str()
                .unwrap()
                .contains("const x = 1;")
        );
        let semantic = &review["result"]["semantic"];
        assert_eq!(semantic["provider"], "host");
        assert_eq!(semantic["model"], "the-clients-model");
        assert_eq!(semantic["detected"], 1);
        let findings = review["result"]["findingsPage"]["findings"]
            .as_array()
            .unwrap();
        assert!(
            findings
                .iter()
                .any(|f| f["explanation"].as_str().unwrap().contains("suspicion"))
        );
        daemon.finish();
    }

    #[test]
    fn a_client_that_declines_leaves_the_review_whole() {
        let repo = repo("cm-decline");
        let mut daemon = Daemon::start(&repo, &["--semantic", "host"]);
        daemon.hello_offering_a_model();
        let (review, asked) = daemon.request_and_answer(
            "review/run",
            files_review(),
            |_| json!({ "declined": true }),
        );
        assert_eq!(review["ok"], true, "{review}");
        assert_ne!(asked.len(), 0);
        assert_eq!(review["result"]["semantic"]["detected"], 0);
        assert!(review["result"]["semantic"]["failed"].as_u64().unwrap() > 0);
        daemon.finish();
    }

    #[test]
    fn cancelling_while_the_client_thinks_ends_the_review() {
        let repo = repo("cm-cancel");
        let mut daemon = Daemon::start(&repo, &["--semantic", "host"]);
        daemon.hello_offering_a_model();
        daemon.next += 1;
        let id = format!("c-{}", daemon.next);
        daemon.send_raw(
            &json!({ "v": 1, "type": "request", "id": id, "method": "review/run", "params": files_review() })
                .to_string(),
        );
        let asked = loop {
            let message = daemon.read();
            if message["type"] == "request" {
                break message;
            }
        };
        assert_eq!(asked["method"], "model/complete");
        daemon.send_raw(
            &json!({ "v": 1, "type": "event", "method": "cancel", "params": { "target": id } })
                .to_string(),
        );
        let response = daemon.response_to(&id);
        assert_eq!(response["error"]["code"], "cancelled");
        daemon.finish();
    }

    impl Daemon {
        fn hello_that_resumes(&mut self) -> Value {
            self.request(
                "hello",
                json!({ "client": { "name": "t", "version": "0" }, "protocol": { "min": 1, "max": 1 },
                        "capabilities": { "inputRequired": true } }),
            )
        }

        /// Runs a review and, each time it asks for input, resumes it with `answer`. Returns the
        /// final response and the inputs it asked for.
        fn run_resuming(
            &mut self,
            params: Value,
            answer: impl Fn(&Value) -> Value,
        ) -> (Value, Vec<Value>) {
            let mut response = self.request("review/run", params);
            let mut inputs = Vec::new();
            while response["result"]["status"] == "input_required" {
                let token = response["result"]["token"].clone();
                let mut answers = serde_json::Map::new();
                for input in response["result"]["inputs"].as_array().unwrap() {
                    answers.insert(input["id"].as_str().unwrap().to_owned(), answer(input));
                    inputs.push(input.clone());
                }
                response = self.request(
                    "review/resume",
                    json!({ "token": token, "responses": answers }),
                );
            }
            (response, inputs)
        }
    }

    #[test]
    fn a_review_that_cannot_be_asked_stops_and_resumes_with_the_answer() {
        let repo = repo("cm-resume");
        let mut daemon = Daemon::start(&repo, &["--semantic", "host"]);
        let hello = daemon.hello_that_resumes();
        assert_eq!(hello["result"]["capabilities"]["semantic"], json!(["host"]));
        let params = json!({ "scope": { "kind": "files", "paths": ["a.js"] }, "semantic": true });

        let first = daemon.request("review/run", params.clone());
        assert_eq!(first["result"]["status"], "input_required", "{first}");
        let input = &first["result"]["inputs"][0];
        assert_eq!(input["kind"], "model/complete");
        assert!(
            input["params"]["user"]
                .as_str()
                .unwrap()
                .contains("const x = 1;")
        );
        assert!(
            !repo.0.join(".debuggatha/ledger.json").exists(),
            "nothing is written while the review waits"
        );
        let resumed = daemon.request(
            "review/resume",
            json!({ "token": first["result"]["token"],
                    "responses": { input["id"].as_str().unwrap(): { "text": SUSPICION, "model": "m" } } }),
        );
        // The answer to the resume is the review's end, or the next prompt it needs.
        assert!(resumed["ok"] == true, "{resumed}");
        daemon.finish();

        let mut again = Daemon::start(&repo, &["--semantic", "host"]);
        again.hello_that_resumes();
        let (review, inputs) = again.run_resuming(
            params,
            |_| json!({ "text": SUSPICION, "model": "the-clients-model" }),
        );
        assert_ne!(inputs.len(), 0);
        assert_eq!(review["ok"], true, "{review}");
        assert_eq!(review["result"]["semantic"]["provider"], "host");
        assert_eq!(review["result"]["semantic"]["model"], "the-clients-model");
        assert_eq!(review["result"]["semantic"]["detected"], 1);
        assert!(repo.0.join(".debuggatha/ledger.json").exists());
        again.finish();
    }

    #[test]
    fn declining_a_prompt_leaves_the_review_whole() {
        let repo = repo("cm-resume-decline");
        let mut daemon = Daemon::start(&repo, &["--semantic", "host"]);
        daemon.hello_that_resumes();
        let (review, inputs) = daemon.run_resuming(files_review(), |_| json!({ "declined": true }));
        assert_ne!(inputs.len(), 0);
        assert_eq!(review["ok"], true, "{review}");
        assert_eq!(review["result"]["semantic"]["detected"], 0);
        daemon.finish();
    }

    #[test]
    fn a_resume_after_the_repository_changed_is_a_conflict_and_writes_nothing() {
        let repo = repo("cm-resume-conflict");
        let mut daemon = Daemon::start(&repo, &["--semantic", "host"]);
        daemon.hello_that_resumes();
        let params = json!({ "scope": { "kind": "files", "paths": ["a.js"] }, "semantic": true });
        let first = daemon.request("review/run", params);
        assert_eq!(first["result"]["status"], "input_required");
        std::fs::write(repo.0.join("a.js"), "const x = 1;\nconst y = 2;\n").unwrap();
        let resumed = daemon.request(
            "review/resume",
            json!({ "token": first["result"]["token"], "responses": {} }),
        );
        assert_eq!(resumed["error"]["code"], "conflict");
        std::thread::sleep(std::time::Duration::from_millis(500));
        assert!(
            !repo.0.join(".debuggatha/ledger.json").exists(),
            "a review the client lost does not write"
        );
        let unknown = daemon.request(
            "review/resume",
            json!({ "token": first["result"]["token"], "responses": {} }),
        );
        assert_eq!(unknown["error"]["code"], "not_found", "the token is spent");
        daemon.finish();
    }

    #[test]
    fn a_client_that_can_be_asked_is_asked_and_one_that_cannot_resume_is_not_suspended() {
        let repo = repo("cm-neither");
        let mut daemon = Daemon::start(&repo, &["--semantic", "host"]);
        daemon.hello();
        let review = daemon.request("review/run", files_review());
        assert_eq!(review["ok"], true, "{review}");
        assert_eq!(
            review["result"]["semantic"],
            Value::Null,
            "a client with no model gets a review with none"
        );
        let nobody = daemon.request("review/resume", json!({ "token": "r-1", "responses": {} }));
        assert_eq!(nobody["error"]["code"], "not_found");
        daemon.finish();
    }
}

#[test]
fn a_finding_is_explained_and_accepted_with_a_reason_that_the_next_review_honors() {
    let repo = Repo::new("explain");
    let mut daemon = Daemon::start(&repo, &[]);
    daemon.hello();
    let first = daemon.request("review/run", serde_json::from_str(EVAL).unwrap());
    let before = first["result"]["findings"].as_u64().unwrap();
    assert_ne!(before, 0);
    let listed = daemon.request("findings/list", json!({}));
    let entry = &listed["result"]["entries"][0];
    let id = entry["id"].clone();

    let explained = daemon.request("findings/explain", json!({ "id": id }));
    assert_eq!(explained["ok"], true, "{explained}");
    let rule = explained["result"]["rule"]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    assert_ne!(explained["result"]["why"].as_str().unwrap().len(), 0);
    assert!(
        explained["result"]["ifYouAcceptIt"]["inlineComment"]
            .as_str()
            .unwrap()
            .contains(&rule)
    );
    assert_eq!(explained["result"]["finding"]["file"], "a.js");

    let short = daemon.request("findings/suppress", json!({ "id": id, "reason": "no" }));
    assert_eq!(
        short["error"]["code"], "invalid_request",
        "a reason is required"
    );
    let missing = daemon.request(
        "findings/suppress",
        json!({ "id": "nope", "reason": "the input is a constant here" }),
    );
    assert_eq!(missing["error"]["code"], "not_found");

    let accepted = daemon.request(
        "findings/suppress",
        json!({ "id": id, "reason": "the input is a constant here" }),
    );
    assert_eq!(accepted["ok"], true, "{accepted}");
    assert_eq!(accepted["result"]["ruleId"], rule.as_str());
    let got = daemon.request("findings/get", json!({ "id": id }));
    assert_eq!(got["result"]["entry"]["status"], "dismissed");
    let memory = daemon.request("memory/list", json!({}));
    assert!(
        memory["result"]
            .to_string()
            .contains("the input is a constant here")
    );

    let again = daemon.request("review/run", serde_json::from_str(EVAL).unwrap());
    assert!(
        again["result"]["findings"].as_u64().unwrap() < before,
        "the accepted rule is no longer reported in that file: {again}"
    );
    daemon.finish();
}
