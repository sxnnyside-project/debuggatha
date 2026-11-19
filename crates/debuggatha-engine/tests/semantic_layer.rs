//! The semantic layer only adds opinions, and a model's claim is kept only if it can be checked.
#![allow(clippy::expect_used, clippy::unwrap_used)] // a failed expect is a failed test

use std::cell::RefCell;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;

use debuggatha_core::env::Env;
use debuggatha_core::review_engine::{
    Confidence, CreateFindingInput, Evidence, Finding, FindingLocation, LineRange, SemanticRole,
    SemanticVerdict, Severity, create_finding,
};
use debuggatha_engine::diff::parse_unified_diff;
use debuggatha_engine::semantic_layer::{
    CompletionRequest, LocalRuntime, Refine, Scope, SemanticOptions, SemanticProvider,
    assert_loopback, connect_local, refine_findings,
};

struct FixedEnv;
impl Env for FixedEnv {
    fn now(&self) -> String {
        "2026-01-01T00:00:00.000Z".into()
    }
    fn new_id(&self) -> String {
        "id".into()
    }
}

/// Answers from a queue and remembers every prompt it was sent.
struct Fake {
    replies: RefCell<Vec<Result<String, String>>>,
    seen: RefCell<Vec<CompletionRequest>>,
}

impl Fake {
    fn new(replies: Vec<Result<&str, &str>>) -> Self {
        Self {
            replies: RefCell::new(
                replies
                    .into_iter()
                    .rev()
                    .map(|r| r.map(str::to_owned).map_err(str::to_owned))
                    .collect(),
            ),
            seen: RefCell::default(),
        }
    }

    fn prompts(&self) -> String {
        self.seen
            .borrow()
            .iter()
            .map(|r| r.user.clone())
            .collect::<Vec<_>>()
            .join("\n---\n")
    }
}

impl SemanticProvider for Fake {
    fn name(&self) -> &'static str {
        "fake"
    }
    fn model(&self) -> Option<String> {
        Some("m1".into())
    }
    fn complete(&self, request: &CompletionRequest) -> Result<String, String> {
        self.seen.borrow_mut().push(request.clone());
        self.replies
            .borrow_mut()
            .pop()
            .unwrap_or_else(|| Err("no more replies".into()))
    }
}

struct Repo(PathBuf);
impl Repo {
    fn new(name: &str) -> Self {
        let dir =
            std::env::temp_dir().join(format!("debuggatha-sem-{name}-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }
    fn write(&self, path: &str, text: &str) {
        std::fs::write(self.0.join(path), text).unwrap();
    }
}
impl Drop for Repo {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).ok();
    }
}

fn finding(
    file: &str,
    line: u32,
    rule: &str,
    confidence: Confidence,
    severity: Severity,
) -> Finding {
    create_finding(
        CreateFindingInput {
            id: Some(format!("f-{file}-{line}-{rule}")),
            title: format!("{rule} here"),
            explanation: "x".into(),
            severity,
            confidence,
            category: "reliability".into(),
            locations: vec![FindingLocation {
                file: file.into(),
                lines: Some(LineRange {
                    start: line,
                    end: line,
                }),
                columns: None,
            }],
            evidence: vec![
                Evidence::Code {
                    file: file.into(),
                    lines: Some(LineRange {
                        start: line,
                        end: line,
                    }),
                    excerpt: None,
                },
                Evidence::ReviewPackRule {
                    pack_id: "p".into(),
                    rule_id: rule.into(),
                },
            ],
            applied_policy: None,
            originating_pack: None,
            recommendations: vec![],
        },
        &FixedEnv,
    )
    .unwrap()
}

fn refine(
    repo: &Repo,
    scope: Scope<'_>,
    findings: Vec<Finding>,
    options: &SemanticOptions,
    provider: &Fake,
) -> (Vec<Finding>, debuggatha_engine::semantic_layer::SemanticRun) {
    refine_findings(Refine {
        root: &repo.0,
        scope,
        findings,
        policy_id: "p",
        options,
        provider,
        env: &FixedEnv,
    })
    .unwrap()
}

const CODE: &str = "const a = read();\nif (!ok(a)) return;\nuse(a);\nrun(a);\n";

fn verify_only() -> SemanticOptions {
    SemanticOptions {
        verify: true,
        detect: false,
        ..SemanticOptions::default()
    }
}

fn verdict_of(f: &Finding) -> Option<SemanticVerdict> {
    f.evidence.iter().find_map(|e| match e {
        Evidence::SemanticReview {
            role: SemanticRole::Verification,
            verdict,
            ..
        } => *verdict,
        _ => None,
    })
}

#[test]
fn a_verdict_is_evidence_and_never_removes_or_changes_a_finding() {
    let repo = Repo::new("verdict");
    repo.write("a.js", CODE);
    let original = finding("a.js", 3, "unchecked", Confidence::Medium, Severity::Medium);
    let provider = Fake::new(vec![Ok(
        r#"{"verdict":"false_positive","proof":"if (!ok(a)) return;","reason":"checked above"}"#,
    )]);
    let (findings, run) = refine(
        &repo,
        Scope::Workspace,
        vec![original.clone()],
        &verify_only(),
        &provider,
    );

    assert_eq!(findings.len(), 1);
    assert_eq!(verdict_of(&findings[0]), Some(SemanticVerdict::Doubtful));
    assert_eq!(
        (findings[0].severity, findings[0].confidence),
        (original.severity, original.confidence)
    );
    assert_eq!(findings[0].evidence[..2], original.evidence[..2]);
    assert_eq!(
        (run.verified, run.doubtful, run.provider.as_str()),
        (1, 1, "fake")
    );
}

#[test]
fn a_false_positive_claim_needs_a_line_that_proves_it() {
    let repo = Repo::new("proof");
    repo.write("a.js", CODE);
    for proof in ["", "use(a);", "totally invented line", "ok"] {
        let reply = format!(r#"{{"verdict":"false_positive","proof":"{proof}","reason":"fine"}}"#);
        let provider = Fake::new(vec![Ok(&reply)]);
        let f = finding("a.js", 3, "unchecked", Confidence::Medium, Severity::Medium);
        let (findings, run) = refine(&repo, Scope::Workspace, vec![f], &verify_only(), &provider);
        assert_eq!(
            verdict_of(&findings[0]),
            Some(SemanticVerdict::Unsure),
            "proof {proof:?}"
        );
        assert_eq!(run.doubtful, 0);
    }
}

#[test]
fn only_findings_the_detectors_are_unsure_of_are_verified_and_most_severe_first() {
    let repo = Repo::new("candidates");
    repo.write("a.js", CODE);
    let sure = finding("a.js", 1, "r1", Confidence::High, Severity::Critical);
    let low = finding("a.js", 2, "r2", Confidence::Low, Severity::Low);
    let high = finding("a.js", 3, "r3", Confidence::Medium, Severity::High);
    let provider = Fake::new(vec![
        Ok(r#"{"verdict":"real","reason":"a"}"#),
        Ok(r#"{"verdict":"real","reason":"b"}"#),
    ]);
    let options = SemanticOptions {
        max_candidates: 1,
        ..verify_only()
    };
    let (findings, run) = refine(
        &repo,
        Scope::Workspace,
        vec![sure, low, high],
        &options,
        &provider,
    );

    assert_eq!(run.verified, 1);
    assert_eq!(provider.seen.borrow().len(), 1);
    assert!(
        provider.prompts().contains("r3 here"),
        "the high-severity candidate is the one asked about"
    );
    assert_eq!(
        findings.iter().filter(|f| verdict_of(f).is_some()).count(),
        1
    );
}

#[test]
fn nothing_that_may_hold_a_secret_reaches_the_model() {
    let repo = Repo::new("secrets");
    repo.write(
        "a.js",
        "const k = \"mock_secret_abcdefghijklmnopqrstuvwxyz\";\nuse(k);\nrun();\n",
    );
    repo.write(".env", "TOKEN=abc\n");
    repo.write("c.js", "const t = \"AKIAABCDEFGHIJKLMNOP1234\";\nrun(t);\n");
    let secret = finding(
        "c.js",
        1,
        "no-hardcoded-secrets",
        Confidence::Medium,
        Severity::High,
    );
    let in_env = finding(".env", 1, "other", Confidence::Medium, Severity::Medium);
    let plain = finding("a.js", 2, "other", Confidence::Medium, Severity::Medium);
    let provider = Fake::new(vec![Ok(r#"{"verdict":"real","reason":"r"}"#); 3]);
    refine(
        &repo,
        Scope::Workspace,
        vec![secret, in_env, plain],
        &verify_only(),
        &provider,
    );

    let prompts = provider.prompts();
    assert!(
        !prompts.contains("mock_secret_"),
        "a long quoted token is scrubbed: {prompts}"
    );
    assert!(
        !prompts.contains("TOKEN=abc"),
        "a secret file is never read"
    );
    assert!(
        !prompts.contains("AKIA"),
        "a finding about a secret is not verified and its line is withheld"
    );
    assert_eq!(
        provider.seen.borrow().len(),
        1,
        "only the plain finding was asked about"
    );
}

#[test]
fn a_detection_is_kept_only_if_it_quotes_its_line() {
    let repo = Repo::new("detect");
    repo.write("a.js", CODE);
    let reply = r#"{"issues":[
        {"line":3,"quote":"use(a);","title":"Use after failed check","why":"a may be unset.","category":"reliability","severity":"low"},
        {"line":4,"quote":"launch(missiles)","title":"Invented","why":"The quote is not on the line.","category":"security","severity":"medium"},
        {"line":99,"quote":"use(a);","title":"No such line","why":"x","category":"security","severity":"medium"},
        {"line":2,"quote":"if (!ok(a)) return;","title":"","why":"no title","category":"security"}
    ]}"#;
    let provider = Fake::new(vec![Ok(reply)]);
    let files = ["a.js".to_owned()];
    let (findings, run) = refine(
        &repo,
        Scope::Files(&files),
        vec![],
        &SemanticOptions::default(),
        &provider,
    );

    assert_eq!((run.detected, run.rejected), (1, 3));
    let f = &findings[0];
    assert_eq!((f.confidence, f.severity), (Confidence::Low, Severity::Low));
    assert_eq!(f.locations[0].lines.unwrap().start, 3);
    assert!(f.explanation.contains("suspicion to check"));
    assert!(f.evidence.iter().any(|e| matches!(e, Evidence::SemanticReview { role: SemanticRole::Detection, topic: Some(t), .. } if t == "use-after-failed-check")));
}

#[test]
fn a_detection_on_a_line_a_detector_already_flagged_is_not_said_again() {
    let repo = Repo::new("dedupe");
    repo.write("a.js", CODE);
    let existing = finding("a.js", 3, "r", Confidence::High, Severity::High);
    let reply = r#"{"issues":[{"line":3,"quote":"use(a);","title":"Same line","why":"w","category":"reliability","severity":"medium"}]}"#;
    let provider = Fake::new(vec![Ok(reply)]);
    let files = ["a.js".to_owned()];
    let (findings, run) = refine(
        &repo,
        Scope::Files(&files),
        vec![existing],
        &SemanticOptions::default(),
        &provider,
    );
    assert_eq!((findings.len(), run.detected), (1, 0));
}

#[test]
fn a_diff_shows_the_model_only_the_lines_it_added() {
    let repo = Repo::new("diff");
    let diff = parse_unified_diff(
        "diff --git a/a.js b/a.js\n--- a/a.js\n+++ b/a.js\n@@ -1,2 +1,3 @@\n const old = 1;\n+const fresh = dangerous(input);\n const tail = 2;\n",
    );
    let provider = Fake::new(vec![Ok(r#"{"issues":[]}"#)]);
    let (_, run) = refine(
        &repo,
        Scope::Diff(&diff),
        vec![],
        &SemanticOptions::default(),
        &provider,
    );
    let prompts = provider.prompts();
    assert!(
        prompts.contains("   2 | const fresh = dangerous(input);"),
        "{prompts}"
    );
    assert!(!prompts.contains("const old") && !prompts.contains("const tail"));
    assert_eq!(run.failed, 0);
}

#[test]
fn a_model_that_keeps_failing_is_given_up_on_and_the_review_is_untouched() {
    let repo = Repo::new("fail");
    for name in ["a.js", "b.js", "c.js"] {
        repo.write(name, CODE);
    }
    let original = vec![finding("a.js", 1, "r", Confidence::High, Severity::Medium)];
    let provider = Fake::new(vec![
        Err("down"),
        Err("down"),
        Err("down"),
        Ok(r#"{"issues":[]}"#),
    ]);
    let files = ["a.js".to_owned(), "b.js".to_owned(), "c.js".to_owned()];
    let options = SemanticOptions {
        max_detect_files: 5,
        ..SemanticOptions::default()
    };
    let (findings, run) = refine(
        &repo,
        Scope::Files(&files),
        original.clone(),
        &options,
        &provider,
    );

    assert_eq!(findings, original);
    assert_eq!(run.failed, 3);
    assert_eq!(provider.seen.borrow().len(), 3);
    assert!(
        run.notes.iter().any(|n| n.contains("failed repeatedly")),
        "{:?}",
        run.notes
    );
}

#[test]
fn a_workspace_review_says_it_has_no_changed_code() {
    let repo = Repo::new("workspace");
    let provider = Fake::new(vec![]);
    let (_, run) = refine(
        &repo,
        Scope::Workspace,
        vec![],
        &SemanticOptions::default(),
        &provider,
    );
    assert!(run.notes[0].contains("whole-repository review"));
    assert_eq!(provider.seen.borrow().len(), 0);
}

#[test]
fn code_goes_only_to_this_machine() {
    for ok in [
        "http://localhost:11434",
        "http://127.0.0.1:1234/v1",
        "http://[::1]:8080",
    ] {
        assert!(assert_loopback(ok).is_ok(), "{ok}");
    }
    for bad in [
        "http://example.com",
        "https://localhost:11434",
        "http://localhost.evil.com",
        "http://127.0.0.1.evil.com:80",
        "http://user@evil.com",
        "http://0.0.0.0:80",
        "ftp://localhost",
        "localhost",
    ] {
        assert!(assert_loopback(bad).is_err(), "{bad}");
    }
}

/// One request answered with a canned response; returns what was received.
fn serve(responses: Vec<String>) -> (String, std::thread::JoinHandle<Vec<String>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
    let handle = std::thread::spawn(move || {
        let mut received = Vec::new();
        for response in responses {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buffer = vec![0_u8; 65536];
            let mut length = 0;
            loop {
                let n = stream.read(&mut buffer[length..]).unwrap();
                length += n;
                let text = String::from_utf8_lossy(&buffer[..length]).into_owned();
                let body_start = text.find("\r\n\r\n").map(|i| i + 4);
                let wanted = text
                    .lines()
                    .find_map(|l| {
                        l.to_lowercase()
                            .strip_prefix("content-length:")
                            .map(|v| v.trim().parse::<usize>().unwrap())
                    })
                    .unwrap_or(0);
                if n == 0 || body_start.is_some_and(|s| length >= s + wanted) {
                    break;
                }
            }
            received.push(String::from_utf8_lossy(&buffer[..length]).into_owned());
            stream.write_all(response.as_bytes()).unwrap();
        }
        received
    });
    (url, handle)
}

fn http(status: &str, body: &str) -> String {
    format!(
        "HTTP/1.1 {status}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
        body.len()
    )
}

#[test]
fn a_local_ollama_is_asked_deterministically_and_its_model_is_discovered() {
    let (url, server) = serve(vec![
        http("200 OK", r#"{"models":[{"name":"qwen2.5-coder:7b"}]}"#),
        http("200 OK", r#"{"response":"{\"issues\":[]}","done":true}"#),
    ]);
    let provider = connect_local(LocalRuntime::Ollama, Some(&url), None, None).unwrap();
    assert_eq!(
        (provider.name(), provider.model().as_deref()),
        ("ollama", Some("qwen2.5-coder:7b"))
    );
    let reply = provider
        .complete(&CompletionRequest {
            system: "sys".into(),
            user: "usr".into(),
            max_tokens: 50,
        })
        .unwrap();
    assert_eq!(reply, r#"{"issues":[]}"#);

    let received = server.join().unwrap();
    assert!(received[0].starts_with("GET /api/tags"));
    let body = received[1].split("\r\n\r\n").nth(1).unwrap();
    let sent: serde_json::Value = serde_json::from_str(body).unwrap();
    assert_eq!(sent["stream"], false);
    assert_eq!(sent["format"], "json");
    assert_eq!(sent["options"]["temperature"], 0);
    assert_eq!(
        (sent["system"].as_str(), sent["prompt"].as_str()),
        (Some("sys"), Some("usr"))
    );
}

#[test]
fn a_local_lm_studio_uses_the_chat_endpoint() {
    let (url, server) = serve(vec![http(
        "200 OK",
        r#"{"choices":[{"message":{"content":"hello"}}]}"#,
    )]);
    let provider = connect_local(LocalRuntime::LmStudio, Some(&url), Some("m"), None).unwrap();
    let reply = provider
        .complete(&CompletionRequest {
            system: "s".into(),
            user: "u".into(),
            max_tokens: 9,
        })
        .unwrap();
    assert_eq!(reply, "hello");
    let received = server.join().unwrap();
    assert!(received[0].starts_with("POST /v1/chat/completions"));
    assert!(received[0].contains(r#""max_tokens":9"#));
}

#[test]
fn a_redirect_never_sends_the_code_elsewhere_and_errors_do_not_leak() {
    let redirect = "HTTP/1.1 302 Found\r\nlocation: http://example.com/x\r\ncontent-length: 0\r\nconnection: close\r\n\r\n".to_owned();
    let (url, server) = serve(vec![redirect, http("500 Internal Server Error", "boom")]);
    let provider = connect_local(LocalRuntime::Ollama, Some(&url), Some("m"), None).unwrap();
    let request = CompletionRequest {
        system: "s".into(),
        user: "secret code".into(),
        max_tokens: 9,
    };
    assert!(provider.complete(&request).is_err());
    let error = provider.complete(&request).unwrap_err();
    assert_eq!(error, "Ollama request failed with HTTP 500");
    assert_eq!(server.join().unwrap().len(), 2);
}

#[test]
fn a_runtime_that_is_not_there_is_reported_not_guessed() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    let error = connect_local(
        LocalRuntime::Ollama,
        Some(&format!("http://127.0.0.1:{port}")),
        None,
        None,
    )
    .err()
    .unwrap();
    assert!(
        error.starts_with("Ollama is not answering at http://127.0.0.1:"),
        "{error}"
    );
}

#[test]
fn a_line_a_finding_says_holds_a_secret_is_withheld_from_detection_too() {
    let repo = Repo::new("withheld");
    repo.write("c.js", "const t = \"AKIAABCDEFGHIJKLMNOP\";\nrun(t);\n");
    let secret = finding(
        "c.js",
        1,
        "no-hardcoded-secrets",
        Confidence::High,
        Severity::High,
    );
    let provider = Fake::new(vec![Ok(r#"{"issues":[]}"#)]);
    let files = ["c.js".to_owned()];
    refine(
        &repo,
        Scope::Files(&files),
        vec![secret],
        &SemanticOptions::default(),
        &provider,
    );
    let prompts = provider.prompts();
    assert!(
        prompts.contains("1 | [line withheld: may hold a secret]"),
        "{prompts}"
    );
    assert!(!prompts.contains("AKIA"));
    assert!(prompts.contains("run(t);"));
}
