//! A whole review against a real directory: the ledger follows the code, the baseline hides what
//! it accepted, memory is read, and the three scopes share one pipeline.
#![allow(clippy::expect_used, clippy::unwrap_used)] // a failed expect is a failed test

use std::path::PathBuf;

use debuggatha_core::env::SystemEnv;
use debuggatha_core::review_engine::ReviewDepth;
use debuggatha_engine::pipeline::{ReviewInput, Scope, execute_review};

struct Repo(PathBuf);

impl Repo {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("debuggatha-{name}-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("package.json"),
            r#"{"name":"t","version":"1.0.0","dependencies":{"express":"4.0.0"}}"#,
        )
        .unwrap();
        Self(dir)
    }

    fn write(&self, name: &str, text: &str) {
        std::fs::write(self.0.join(name), text).unwrap();
    }

    fn review(&self, scope: Scope) -> debuggatha_engine::pipeline::ReviewOutput {
        execute_review(
            &ReviewInput {
                root: &self.0,
                scope,
                depth: ReviewDepth::Full,
                pack_ids: vec![],
                policy_id: None,
                source_name: "test",
                persist: true,
                include_baselined: false,
                analyzers: None,
                analyzer_search: vec![],
                semantic: None,
            },
            &SystemEnv,
        )
        .expect("review")
    }
}

impl Drop for Repo {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).ok();
    }
}

#[test]
fn the_ledger_follows_the_code() {
    let repo = Repo::new("pipeline-ledger");
    repo.write("a.js", "const x = eval(input);\n");

    let first = repo.review(Scope::Workspace);
    assert_eq!(first.result.findings.len(), 1);
    assert_eq!(first.changes.introduced.len(), 1);
    assert_eq!(first.changes.introduced[0].file, "a.js");

    let again = repo.review(Scope::Workspace);
    assert_eq!(again.changes.introduced.len(), 0, "already tracked");

    repo.write("a.js", "const x = JSON.parse(input);\n");
    let fixed = repo.review(Scope::Workspace);
    assert_eq!(fixed.result.findings.len(), 0);
    assert_eq!(fixed.changes.fixed.len(), 1);

    repo.write("a.js", "const x = eval(input);\n");
    let back = repo.review(Scope::Workspace);
    assert_eq!(back.changes.reopened.len(), 1);
}

#[test]
fn a_file_review_names_files_either_way_and_resolves_only_those() {
    let repo = Repo::new("pipeline-files");
    repo.write("a.js", "const x = eval(input);\n");
    repo.write("b.js", "const y = eval(input);\n");
    repo.review(Scope::Workspace);

    repo.write("a.js", "const x = 1;\n");
    let absolute = repo.0.join("a.js").to_string_lossy().into_owned();
    let out = repo.review(Scope::Files(vec![absolute]));
    assert_eq!(out.changes.fixed.len(), 1);
    assert_eq!(out.changes.fixed[0].file, "a.js");
    assert_eq!(out.result.findings.len(), 0);

    let rest = repo.review(Scope::Workspace);
    assert_eq!(rest.result.findings.len(), 1, "b.js was not touched");
    assert_eq!(rest.changes.fixed.len(), 0);
}

#[test]
fn a_baseline_hides_what_it_accepted_but_the_ledger_still_tracks_it() {
    let repo = Repo::new("pipeline-baseline");
    repo.write("a.js", "const x = eval(input);\n");
    let all = repo.review(Scope::Workspace);
    let baseline =
        debuggatha_engine::baseline::create_baseline_from(&all.result.findings, &SystemEnv);
    debuggatha_engine::baseline::save_baseline(&repo.0, &baseline).unwrap();

    let hidden = repo.review(Scope::Workspace);
    assert_eq!(hidden.result.findings.len(), 0);
    assert_eq!(hidden.baselined, 1);
    assert_eq!(hidden.changes.introduced.len(), 0);

    repo.write("b.js", "const y = eval(input);\n");
    let fresh = repo.review(Scope::Workspace);
    assert_eq!(fresh.result.findings.len(), 1);
    assert_eq!(fresh.baselined, 1);
}

#[test]
fn an_empty_file_list_and_an_unknown_policy_are_errors_not_empty_reviews() {
    let repo = Repo::new("pipeline-errors");
    let input = |scope, policy_id| ReviewInput {
        root: &repo.0,
        scope,
        depth: ReviewDepth::Full,
        pack_ids: vec![],
        policy_id,
        source_name: "test",
        persist: false,
        include_baselined: false,
        analyzers: None,
        analyzer_search: vec![],
        semantic: None,
    };
    assert!(execute_review(&input(Scope::Files(vec![]), None), &SystemEnv).is_err());
    assert!(execute_review(&input(Scope::Workspace, Some("nope".into())), &SystemEnv).is_err());
}

#[test]
fn a_review_that_does_not_persist_leaves_no_ledger() {
    let repo = Repo::new("pipeline-readonly");
    repo.write("a.js", "const x = eval(input);\n");
    let out = execute_review(
        &ReviewInput {
            root: &repo.0,
            scope: Scope::Workspace,
            depth: ReviewDepth::Full,
            pack_ids: vec![],
            policy_id: None,
            source_name: "test",
            persist: false,
            include_baselined: false,
            analyzers: None,
            analyzer_search: vec![],
            semantic: None,
        },
        &SystemEnv,
    )
    .unwrap();
    assert_eq!(out.result.findings.len(), 1);
    assert!(!repo.0.join(".debuggatha").exists());
}

#[cfg(unix)]
mod analyzers {
    use std::os::unix::fs::PermissionsExt;

    use debuggatha_engine::analyzers::{AnalyzerOptions, Status};

    use super::*;

    const SARIF: &str = r#"{"version":"2.1.0","runs":[{"tool":{"driver":{"name":"gitleaks","rules":[{"id":"generic-api-key"}]}},"results":[{"ruleId":"generic-api-key","level":"error","message":{"text":"A key"},"locations":[{"physicalLocation":{"artifactLocation":{"uri":"a.js"},"region":{"startLine":1}}}]}]}]}"#;

    fn bin_with_gitleaks(repo: &Repo) -> PathBuf {
        let bin = repo.0.with_extension("bin");
        std::fs::create_dir_all(&bin).unwrap();
        let script = format!(
            "#!/bin/sh\nif [ \"$1\" = \"version\" ] || [ \"$1\" = \"--version\" ]; then echo 9.9.9; exit 0; fi\ncat <<'EOF'\n{SARIF}\nEOF\nexit 0\n"
        );
        let path = bin.join("gitleaks");
        std::fs::write(&path, script).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        bin
    }

    fn review_with(repo: &Repo, search: Vec<PathBuf>) -> debuggatha_engine::pipeline::ReviewOutput {
        execute_review(
            &ReviewInput {
                root: &repo.0,
                scope: Scope::Workspace,
                depth: ReviewDepth::Full,
                pack_ids: vec![],
                policy_id: None,
                source_name: "test",
                persist: true,
                include_baselined: false,
                analyzers: Some(AnalyzerOptions::default()),
                analyzer_search: search,
                semantic: None,
            },
            &SystemEnv,
        )
        .unwrap()
    }

    #[test]
    fn an_installed_safe_analyzer_joins_the_review_and_is_reported() {
        let repo = Repo::new("pipeline-analyzer");
        repo.write("a.js", "const x = 1;\n");
        let bin = bin_with_gitleaks(&repo);

        let out = review_with(&repo, vec![bin.clone()]);
        std::fs::remove_dir_all(bin).ok();
        let run = out.analyzers.iter().find(|r| r.id == "gitleaks").unwrap();
        assert_eq!(run.status, Status::Ran);
        assert_eq!(out.result.findings.len(), 1);
        assert!(out.result.findings[0].title.contains("A key"));
    }

    #[test]
    fn an_analyzer_that_did_not_run_does_not_close_its_findings() {
        let repo = Repo::new("pipeline-analyzer-gone");
        repo.write("a.js", "const x = 1;\n");
        let bin = bin_with_gitleaks(&repo);
        review_with(&repo, vec![bin.clone()]);
        std::fs::remove_dir_all(bin).ok();

        let without = review_with(&repo, vec![]);
        assert_eq!(without.changes.fixed.len(), 0, "absence is not a fix");
        assert_eq!(without.sync.auto_resolved_entry_ids.len(), 0);
    }

    #[test]
    fn no_options_run_no_analyzer() {
        let repo = Repo::new("pipeline-no-analyzer");
        repo.write("a.js", "const x = 1;\n");
        assert_eq!(repo.review(Scope::Workspace).analyzers.len(), 0);
    }
}

mod semantic {
    use debuggatha_engine::pipeline::SemanticInput;
    use debuggatha_engine::semantic_layer::{CompletionRequest, SemanticOptions, SemanticProvider};

    use super::*;

    struct Says(&'static str);
    impl SemanticProvider for Says {
        fn name(&self) -> &'static str {
            "fake"
        }
        fn model(&self) -> Option<String> {
            None
        }
        fn complete(&self, _: &CompletionRequest) -> Result<String, String> {
            Ok(self.0.to_owned())
        }
    }

    const SUSPICION: &str = r#"{"issues":[{"line":1,"quote":"const x = 1;","title":"Suspicious","why":"Looks odd.","category":"reliability","severity":"low"}]}"#;

    fn review_with(
        repo: &Repo,
        provider: Option<&Says>,
    ) -> debuggatha_engine::pipeline::ReviewOutput {
        execute_review(
            &ReviewInput {
                root: &repo.0,
                scope: Scope::Workspace,
                depth: ReviewDepth::Full,
                pack_ids: vec![],
                policy_id: None,
                source_name: "test",
                persist: true,
                include_baselined: false,
                analyzers: None,
                analyzer_search: vec![],
                semantic: provider.map(|p| SemanticInput {
                    provider: p,
                    options: SemanticOptions {
                        verify: true,
                        ..SemanticOptions::default()
                    },
                }),
            },
            &SystemEnv,
        )
        .unwrap()
    }

    #[test]
    fn a_model_adds_a_suspicion_and_the_run_is_reported() {
        let repo = Repo::new("pipeline-semantic");
        repo.write("a.js", "const x = 1;\n");
        let out = review_with(&repo, Some(&Says(SUSPICION)));
        assert!(out.semantic.is_some());
        assert_eq!(
            out.result.findings.len(),
            0,
            "a workspace review has no changed code to detect on"
        );

        let files = execute_review(
            &ReviewInput {
                root: &repo.0,
                scope: Scope::Files(vec!["a.js".into()]),
                depth: ReviewDepth::Full,
                pack_ids: vec![],
                policy_id: None,
                source_name: "test",
                persist: true,
                include_baselined: false,
                analyzers: None,
                analyzer_search: vec![],
                semantic: Some(SemanticInput {
                    provider: &Says(SUSPICION),
                    options: SemanticOptions::default(),
                }),
            },
            &SystemEnv,
        )
        .unwrap();
        assert_eq!(files.semantic.unwrap().detected, 1);
        assert_eq!(files.result.findings.len(), 1);
        assert!(
            files.result.findings[0]
                .explanation
                .contains("suspicion to check")
        );

        let without = review_with(&repo, None);
        assert!(without.semantic.is_none());
        assert_eq!(
            without.changes.fixed.len(),
            0,
            "a model never closes a finding by not repeating it"
        );
    }
}
