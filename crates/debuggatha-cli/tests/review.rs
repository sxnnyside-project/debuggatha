//! The binary itself: arguments in, report and exit code out.
#![allow(clippy::expect_used, clippy::unwrap_used)] // a failed expect is a failed test

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

struct Repo(PathBuf);

impl Repo {
    fn new(name: &str, source: &str) -> Self {
        let dir =
            std::env::temp_dir().join(format!("debuggatha-cli-{name}-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("package.json"),
            r#"{"name":"t","version":"1.0.0","dependencies":{"express":"4.0.0"}}"#,
        )
        .unwrap();
        std::fs::write(dir.join("a.js"), source).unwrap();
        Self(dir)
    }

    fn run(&self, args: &[&str]) -> Output {
        run_in(&self.0, args)
    }

    fn git_init(&self) {
        for args in [
            vec!["init", "-q", "-b", "main"],
            vec!["add", "-A"],
            vec![
                "-c",
                "user.name=t",
                "-c",
                "user.email=t@t",
                "-c",
                "commit.gpgsign=false",
                "commit",
                "-q",
                "-m",
                "init",
            ],
        ] {
            let ok = Command::new("git")
                .args(&args)
                .current_dir(&self.0)
                .status()
                .unwrap()
                .success();
            assert!(ok, "git {args:?}");
        }
    }
}

impl Drop for Repo {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).ok();
    }
}

fn run_in(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_debuggatha"))
        .arg("review")
        .arg("--root")
        .arg(root)
        .args(args)
        .output()
        .unwrap()
}

fn code(output: &Output) -> i32 {
    output.status.code().unwrap()
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

#[test]
fn a_finding_exits_2_and_a_clean_repository_exits_0() {
    let dirty = Repo::new("dirty", "const x = eval(input);\n");
    let out = dirty.run(&[]);
    assert_eq!(code(&out), 2, "{}", stdout(&out));
    assert!(stdout(&out).contains("a.js:1"));

    let clean = Repo::new("clean", "const x = 1;\n");
    assert_eq!(code(&clean.run(&[])), 0);
}

#[test]
fn fail_on_sets_the_threshold() {
    let repo = Repo::new("threshold", "const x = eval(input);\n");
    assert_eq!(code(&repo.run(&["--fail-on", "none"])), 0);
    assert_eq!(code(&repo.run(&["--fail-on", "informational"])), 2);
}

#[test]
fn json_is_one_document_with_the_findings_and_the_coverage() {
    let repo = Repo::new("json", "const x = eval(input);\n");
    let out = repo.run(&["--format", "json", "--no-persist"]);
    let value: serde_json::Value = serde_json::from_slice(&out.stdout).expect("json");
    assert_eq!(value["findings"].as_array().unwrap().len(), 1);
    assert!(value["coverage"].is_object());
    assert!(value["policyId"].is_string());
    assert!(!repo.0.join(".debuggatha").exists());
}

#[test]
fn the_default_in_a_git_repository_reviews_what_changed() {
    let repo = Repo::new("changed", "const x = eval(input);\n");
    repo.git_init();
    assert_eq!(
        code(&repo.run(&["--no-persist"])),
        2,
        "nothing changed, so the whole repository is reviewed"
    );
    assert_eq!(
        code(&repo.run(&["--all", "--no-persist"])),
        2,
        "--all reads everything"
    );

    std::fs::write(repo.0.join("b.js"), "const y = eval(input);\n").unwrap();
    let out = repo.run(&["--format", "json", "--no-persist"]);
    let value: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let findings = value["findings"].as_array().unwrap();
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0]["locations"][0]["file"], "b.js");
}

#[test]
fn files_are_reviewed_whole() {
    let repo = Repo::new("files", "const x = eval(input);\n");
    repo.git_init();
    assert_eq!(code(&repo.run(&["--files", "a.js", "--no-persist"])), 2);
}

#[test]
fn errors_exit_1_and_name_no_path() {
    let repo = Repo::new("errors", "const x = 1;\n");
    let missing = run_in(&repo.0.join("nope"), &[]);
    assert_eq!(code(&missing), 1);

    repo.git_init();
    let unsafe_ref = repo.run(&["--base=--output=x"]);
    assert_eq!(code(&unsafe_ref), 1);
    let message = String::from_utf8_lossy(&unsafe_ref.stderr).into_owned();
    assert!(message.contains("not a branch"), "{message}");
    assert!(!message.contains(repo.0.to_str().unwrap()));
}

#[cfg(unix)]
mod analyzers {
    use std::os::unix::fs::PermissionsExt;

    use super::*;

    const SARIF: &str = r#"{"version":"2.1.0","runs":[{"tool":{"driver":{"name":"gitleaks","rules":[{"id":"generic-api-key"}]}},"results":[{"ruleId":"generic-api-key","level":"error","message":{"text":"A key"},"locations":[{"physicalLocation":{"artifactLocation":{"uri":"a.js"},"region":{"startLine":1}}}]}]}]}"#;

    fn with_fake_gitleaks(repo: &Repo, args: &[&str]) -> Output {
        let bin = repo.0.with_extension("bin");
        std::fs::create_dir_all(&bin).unwrap();
        let script = format!(
            "#!/bin/sh\nif [ \"$1\" = \"version\" ] || [ \"$1\" = \"--version\" ]; then echo 9.9.9; exit 0; fi\ncat <<'EOF'\n{SARIF}\nEOF\n"
        );
        let path = bin.join("gitleaks");
        std::fs::write(&path, script).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        let system = std::env::var("PATH").unwrap_or_default();
        let out = Command::new(env!("CARGO_BIN_EXE_debuggatha"))
            .args(["review", "--root"])
            .arg(&repo.0)
            .args(["--no-persist", "--all"])
            .args(args)
            .env("PATH", format!("{}:{system}", bin.display()))
            .output()
            .unwrap();
        std::fs::remove_dir_all(bin).ok();
        out
    }

    #[test]
    fn an_installed_analyzer_runs_by_default_and_can_be_turned_off() {
        let repo = Repo::new("an-on", "const x = 1;\n");
        let out = with_fake_gitleaks(&repo, &[]);
        assert_eq!(code(&out), 2, "{}", stdout(&out));
        assert!(
            stdout(&out).contains("Gitleaks 9.9.9 ran"),
            "{}",
            stdout(&out)
        );
        assert!(stdout(&out).contains("ran: 1 findings"));

        let off = with_fake_gitleaks(&repo, &["--no-analyzers"]);
        assert_eq!(code(&off), 0, "{}", stdout(&off));
    }

    #[test]
    fn the_inventory_lists_every_analyzer() {
        let repo = Repo::new("an-list", "const x = 1;\n");
        let out = Command::new(env!("CARGO_BIN_EXE_debuggatha"))
            .args(["analyzers", "--format", "json", "--root"])
            .arg(&repo.0)
            .output()
            .unwrap();
        assert_eq!(code(&out), 0);
        let value: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(value["analyzers"].as_array().unwrap().len(), 9);
    }

    #[test]
    fn an_unknown_analyzer_is_a_usage_error_with_exit_1_not_2() {
        let repo = Repo::new("an-unknown", "const x = 1;\n");
        let out = repo.run(&["--analyzer", "nope"]);
        assert_eq!(code(&out), 1);
        assert!(String::from_utf8_lossy(&out.stderr).contains("nope"));
    }
}

mod semantic {
    use std::io::{Read, Write};
    use std::net::TcpListener;

    use super::*;

    /// A fake Ollama that answers one generate request with `reply` and returns what it received.
    fn ollama(reply: &str) -> (String, std::thread::JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
        let body = serde_json::json!({ "response": reply }).to_string();
        let handle = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buffer = vec![0_u8; 65536];
            let mut length = 0;
            loop {
                let n = stream.read(&mut buffer[length..]).unwrap();
                length += n;
                let text = String::from_utf8_lossy(&buffer[..length]).into_owned();
                let start = text.find("\r\n\r\n").map(|i| i + 4);
                let wanted = text
                    .lines()
                    .find_map(|l| {
                        l.to_lowercase()
                            .strip_prefix("content-length:")
                            .map(|v| v.trim().parse::<usize>().unwrap())
                    })
                    .unwrap_or(0);
                if n == 0 || start.is_some_and(|s| length >= s + wanted) {
                    break;
                }
            }
            let response = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                body.len()
            );
            stream.write_all(response.as_bytes()).unwrap();
            String::from_utf8_lossy(&buffer[..length]).into_owned()
        });
        (url, handle)
    }

    #[test]
    fn a_local_model_reads_the_code_and_its_suspicion_is_reported() {
        let repo = Repo::new("sem", "const x = 1;\n");
        let reply = r#"{"issues":[{"line":1,"quote":"const x = 1;","title":"Odd","why":"Looks odd.","category":"reliability","severity":"low"}]}"#;
        let (url, server) = ollama(reply);
        let out = repo.run(&[
            "--files",
            "a.js",
            "--no-persist",
            "--semantic",
            "ollama",
            "--semantic-model",
            "m",
            "--semantic-url",
            &url,
        ]);
        let received = server.join().unwrap();
        assert!(
            received.contains("const x = 1;"),
            "the code reached the model"
        );
        assert_eq!(code(&out), 2, "{}", stdout(&out));
        assert!(stdout(&out).contains("Semantic pass (ollama m): 1 suspected"));
    }

    #[test]
    fn a_remote_url_is_refused_before_any_code_leaves() {
        let repo = Repo::new("sem-remote", "const x = 1;\n");
        let out = repo.run(&[
            "--semantic",
            "ollama",
            "--semantic-model",
            "m",
            "--semantic-url",
            "http://example.com:11434",
        ]);
        assert_eq!(code(&out), 1);
        assert!(!String::from_utf8_lossy(&out.stderr).is_empty());
    }

    #[test]
    fn a_semantic_option_without_the_model_switch_is_a_usage_error() {
        let repo = Repo::new("sem-orphan", "const x = 1;\n");
        assert_eq!(code(&repo.run(&["--semantic-verify"])), 1);
    }
}

mod options {
    use std::io::Write;
    use std::process::Stdio;

    use super::*;

    fn write(repo: &Repo, name: &str, text: &str) {
        let full = repo.0.join(name);
        std::fs::create_dir_all(full.parent().unwrap()).unwrap();
        std::fs::write(full, text).unwrap();
    }

    fn git(repo: &Repo, args: &[&str]) -> String {
        let out = Command::new("git")
            .args([
                "-c",
                "user.name=t",
                "-c",
                "user.email=t@t",
                "-c",
                "commit.gpgsign=false",
            ])
            .args(args)
            .current_dir(&repo.0)
            .output()
            .unwrap();
        assert!(out.status.success(), "git {args:?}");
        String::from_utf8_lossy(&out.stdout).into_owned()
    }

    #[test]
    fn every_report_format_says_the_same_finding() {
        let repo = Repo::new("formats", "const x = eval(input);\n");
        let report = |format: &str| {
            let out = repo.run(&["--all", "--no-persist", "--format", format]);
            assert_eq!(code(&out), 2, "{format}");
            stdout(&out)
        };

        assert!(report("github").starts_with("::error file=a.js,line=1,"));
        let sarif: serde_json::Value = serde_json::from_str(&report("sarif")).unwrap();
        assert_eq!(sarif["version"], "2.1.0");
        assert_eq!(sarif["runs"][0]["results"].as_array().unwrap().len(), 1);
        let gitlab: serde_json::Value = serde_json::from_str(&report("gitlab")).unwrap();
        assert_eq!(gitlab[0]["location"]["path"], "a.js");
        assert_eq!(gitlab[0]["fingerprint"].as_str().unwrap().len(), 32);
        let markdown = report("markdown");
        assert!(markdown.starts_with("<!-- debuggatha-report -->"));
        assert!(
            markdown.contains("| critical")
                || markdown.contains("| high")
                || markdown.contains("| medium")
        );
    }

    #[test]
    fn the_report_can_go_to_a_file_and_quiet_leaves_only_the_findings() {
        let repo = Repo::new("output", "const x = eval(input);\n");
        let file = repo.0.join("report.md");
        let out = repo.run(&[
            "--all",
            "--no-persist",
            "--format",
            "markdown",
            "--output",
            file.to_str().unwrap(),
        ]);
        assert_eq!(code(&out), 2);
        assert_eq!(stdout(&out), "", "the report went to the file");
        assert!(
            std::fs::read_to_string(&file)
                .unwrap()
                .contains("Debuggatha review")
        );

        let quiet = repo.run(&["--all", "--no-persist", "--quiet"]);
        assert!(!stdout(&quiet).contains("(policy"), "{}", stdout(&quiet));
        assert!(stdout(&quiet).contains("a.js:1"));
    }

    #[test]
    fn a_diff_on_stdin_makes_only_its_added_lines_findings() {
        let repo = Repo::new("stdin", "const x = eval(input);\n");
        repo.git_init();
        write(
            &repo,
            "a.js",
            "const x = eval(input);\nconst y = eval(other);\n",
        );
        let diff = git(&repo, &["diff", "HEAD"]);

        let mut child = Command::new(env!("CARGO_BIN_EXE_debuggatha"))
            .args([
                "review",
                "--format",
                "json",
                "--no-persist",
                "--diff",
                "-",
                "--root",
            ])
            .arg(&repo.0)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(diff.as_bytes())
            .unwrap();
        let out = child.wait_with_output().unwrap();
        let value: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
        let findings = value["findings"].as_array().unwrap();
        assert_eq!(findings.len(), 1, "line 1 was already there");
        assert_eq!(findings[0]["locations"][0]["lines"]["start"], 2);

        let empty = Command::new(env!("CARGO_BIN_EXE_debuggatha"))
            .args(["review", "--diff", "-", "--root"])
            .arg(&repo.0)
            .stdin(Stdio::null())
            .output()
            .unwrap();
        assert_eq!(code(&empty), 1, "an empty stdin is an error");
    }

    #[test]
    fn changed_since_reviews_whole_changed_files_and_says_when_nothing_changed() {
        let repo = Repo::new("since", "const x = eval(input);\n");
        repo.git_init();
        let none = repo.run(&[
            "--changed-since",
            "HEAD",
            "--format",
            "markdown",
            "--no-persist",
        ]);
        assert_eq!(code(&none), 0);
        assert!(stdout(&none).contains("No changes since HEAD."));

        write(&repo, "b.js", "const y = eval(other);\n");
        let out = repo.run(&[
            "--changed-since",
            "HEAD",
            "--format",
            "json",
            "--no-persist",
        ]);
        let value: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
        let findings = value["findings"].as_array().unwrap();
        assert_eq!(findings.len(), 1, "a.js did not change");
        assert_eq!(findings[0]["locations"][0]["file"], "b.js");
    }

    #[test]
    fn fail_on_new_counts_only_findings_on_changed_lines() {
        let repo = Repo::new("new", "const x = eval(input);\n");
        repo.git_init();
        let old = repo.run(&["--all", "--fail-on-new", "--no-persist"]);
        assert_eq!(code(&old), 0, "{}", stdout(&old));
        assert!(
            stdout(&old).contains("do not affect the result")
                || stdout(&old).contains("does not affect the result")
        );

        write(&repo, "b.js", "const y = eval(other);\n");
        assert_eq!(
            code(&repo.run(&["--all", "--fail-on-new", "--no-persist"])),
            2
        );
    }

    #[test]
    fn the_repository_config_narrows_and_never_enables() {
        let repo = Repo::new("config", "const x = eval(input);\n");
        write(&repo, "legacy/old.js", "const z = eval(old);\n");
        write(
            &repo,
            ".debuggatha/config.json",
            r#"{"ignore":["legacy"],"review":{"failOn":"none"}}"#,
        );
        let out = repo.run(&["--all", "--no-persist", "--format", "json"]);
        assert_eq!(code(&out), 0, "failOn none from the config");
        let value: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
        let files: Vec<_> = value["findings"]
            .as_array()
            .unwrap()
            .iter()
            .map(|f| f["locations"][0]["file"].as_str().unwrap().to_owned())
            .collect();
        assert_eq!(files, ["a.js"], "legacy/ is left out");
        assert_eq!(
            code(&repo.run(&["--all", "--no-persist", "--fail-on", "informational"])),
            2
        );

        write(
            &repo,
            ".debuggatha/config.json",
            r#"{"analyzers":{"enable":["eslint"]}}"#,
        );
        let refused = repo.run(&["--all", "--no-persist"]);
        assert_eq!(code(&refused), 1);
        let message = String::from_utf8_lossy(&refused.stderr).into_owned();
        assert!(message.contains("analyzers.enable"), "{message}");
        assert!(!message.contains(repo.0.to_str().unwrap()), "no path");

        write(&repo, ".debuggatha/config.json", r#"{"semantic":true}"#);
        assert_eq!(code(&repo.run(&["--all"])), 1);
    }

    #[test]
    fn a_path_override_holds_part_of_the_repository_to_another_bar() {
        let repo = Repo::new("override", "const x = eval(input);\n");
        write(
            &repo,
            ".debuggatha/config.json",
            r#"{"review":{"failOn":"critical"},"overrides":[{"paths":["a.js"],"failOn":"informational"}]}"#,
        );
        write(&repo, "other/b.js", "const y = eval(other);\n");
        assert_eq!(
            code(&repo.run(&["--all", "--no-persist"])),
            2,
            "a.js is held to informational"
        );
        write(
            &repo,
            ".debuggatha/config.json",
            r#"{"review":{"failOn":"critical"},"overrides":[{"paths":["nothing/**"],"failOn":"informational"}]}"#,
        );
        assert_eq!(
            code(&repo.run(&["--all", "--no-persist"])),
            0,
            "below critical, no override applies"
        );
    }
}
