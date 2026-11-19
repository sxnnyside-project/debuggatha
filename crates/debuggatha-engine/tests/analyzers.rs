//! Analyzers are separate processes behind a trust gate. The runner is tested against stand-in
//! executables in a directory handed to it, so what it does with a tool (find it, gate it, stop it,
//! survive it) needs no real tool and no change to the process environment. What the real tools
//! print is covered by their recorded output in `tests/fixtures/analyzers`.
#![cfg(unix)]
#![allow(clippy::expect_used, clippy::unwrap_used)] // a failed expect is a failed test

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use debuggatha_core::env::Env;
use debuggatha_core::review_engine::{Evidence, Severity};
use debuggatha_engine::analyzers::{
    ADAPTERS, Adapter, AnalyzerFinding, AnalyzerOptions, BIOME, CLIPPY, DETEKT, ESLINT, GITLEAKS,
    KTLINT, Mode, OSV_SCANNER, PHPSTAN, RUFF, RunInput, SarifOptions, Status, Trust, inventory,
    merge_analyzer_findings, parse_sarif, run_analyzers,
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

fn fixture(name: &str) -> String {
    std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/analyzers")
            .join(name),
    )
    .unwrap()
}

struct Sandbox {
    repo: PathBuf,
    bin: PathBuf,
}

impl Sandbox {
    fn new(name: &str) -> Self {
        let base =
            std::env::temp_dir().join(format!("debuggatha-an-{name}-{}", std::process::id()));
        std::fs::remove_dir_all(&base).ok();
        let (repo, bin) = (base.join("repo"), base.join("bin"));
        std::fs::create_dir_all(&repo).unwrap();
        std::fs::create_dir_all(&bin).unwrap();
        Self { repo, bin }
    }

    fn write(&self, path: &str, text: &str) {
        let full = self.repo.join(path);
        std::fs::create_dir_all(full.parent().unwrap()).unwrap();
        std::fs::write(full, text).unwrap();
    }

    /// An executable that answers `--version`/`version`, records its arguments and prints `stdout`.
    fn install(&self, name: &str, stdout: &str, exit: i32, body: &str) {
        std::fs::write(self.bin.join(format!("{name}.out")), stdout).unwrap();
        let script = format!(
            "#!/bin/sh\nif [ \"$1\" = \"--version\" ] || [ \"$1\" = \"version\" ]; then echo \"9.9.9\"; exit 0; fi\necho \"$@\" > \"{bin}/{name}.args\"\n{body}\ncat \"{bin}/{name}.out\"\nexit {exit}\n",
            bin = self.bin.display()
        );
        let path = self.bin.join(name);
        std::fs::write(&path, script).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    fn run(
        &self,
        options: &AnalyzerOptions,
        files: Option<&[String]>,
    ) -> debuggatha_engine::analyzers::AnalyzerRunResult {
        run_analyzers(&RunInput {
            root: &self.repo,
            files,
            options,
            search: std::slice::from_ref(&self.bin),
            adapters: ADAPTERS,
            control: None,
        })
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        std::fs::remove_dir_all(self.repo.parent().unwrap()).ok();
    }
}

fn enabled(ids: &[&str]) -> AnalyzerOptions {
    AnalyzerOptions {
        enable: ids.iter().map(|s| (*s).to_owned()).collect(),
        ..AnalyzerOptions::default()
    }
}

fn status_of<'a>(
    result: &'a debuggatha_engine::analyzers::AnalyzerRunResult,
    id: &str,
) -> &'a debuggatha_engine::analyzers::AnalyzerRun {
    result.runs.iter().find(|r| r.id == id).unwrap()
}

#[test]
fn recorded_output_is_read_into_findings() {
    let root = Path::new("/repo");
    let leaks = (GITLEAKS.parse)(&fixture("gitleaks.sarif"), root).unwrap();
    let ids: Vec<_> = leaks.iter().map(|f| f.rule_id.as_str()).collect();
    assert_eq!(ids, ["aws-access-token", "github-pat"]);
    assert!(
        leaks
            .iter()
            .all(|f| f.severity == Severity::Critical && f.sensitive)
    );
    assert_eq!(
        (leaks[0].file.as_str(), leaks[0].line),
        ("config.env", Some(1))
    );

    let osv = (OSV_SCANNER.parse)(&fixture("osv-scanner.sarif"), root).unwrap();
    let by_rule = |id: &str| osv.iter().find(|f| f.rule_id == id).unwrap();
    assert_eq!(by_rule("CVE-2021-23337").severity, Severity::High);
    assert_eq!(by_rule("CVE-2020-28500").severity, Severity::Medium);
    assert_eq!(osv[0].file, "package-lock.json");
    assert_eq!(osv[0].line, None);
    assert_eq!(
        (OSV_SCANNER.parse)("", root).unwrap().len(),
        0,
        "nothing to scan"
    );
}

#[test]
fn a_path_outside_the_repository_is_not_a_finding_of_it() {
    let sarif = |uri: &str| {
        format!(
            r#"{{"runs":[{{"results":[{{"ruleId":"r","message":{{"text":"m"}},"locations":[{{"physicalLocation":{{"artifactLocation":{{"uri":"{uri}"}}}}}}]}}]}}]}}"#
        )
    };
    let options = SarifOptions {
        category: |_| "x",
        severity: None,
        sensitive: false,
    };
    for outside in [
        "/etc/passwd",
        "../up.txt",
        "a/../../up.txt",
        "file:///elsewhere/a.py",
    ] {
        let found = parse_sarif(&sarif(outside), Path::new("/repo"), &options).unwrap();
        assert_eq!(found.len(), 0, "{outside}");
    }
    let inside = parse_sarif(
        &sarif("file:///repo/src/a%20b.py"),
        Path::new("/repo"),
        &options,
    )
    .unwrap();
    assert_eq!(inside[0].file, "src/a b.py");
}

#[test]
fn only_safe_analyzers_run_unasked() {
    let sb = Sandbox::new("trust");
    sb.write("package-lock.json", "{}");
    sb.install("gitleaks", &fixture("gitleaks.sarif"), 0, "");
    sb.install(
        "osv-scanner",
        &fixture("osv-scanner.sarif")
            .replace("file:///repo/", &format!("file://{}/", sb.repo.display())),
        1,
        "",
    );

    let result = sb.run(&AnalyzerOptions::default(), None);
    assert_eq!(status_of(&result, "gitleaks").status, Status::Ran);
    assert_eq!(
        status_of(&result, "gitleaks").version.as_deref(),
        Some("9.9.9")
    );
    let osv = status_of(&result, "osv-scanner");
    assert_eq!(osv.status, Status::Skipped);
    assert!(
        osv.reason.as_ref().unwrap().contains("sends package names"),
        "{osv:?}"
    );

    let result = sb.run(&enabled(&["osv-scanner"]), None);
    let osv = status_of(&result, "osv-scanner");
    assert_eq!((osv.status, osv.trust), (Status::Ran, Trust::Network));
    assert!(osv.findings > 0, "exit code 1 means findings, not failure");
}

#[test]
fn the_options_can_only_narrow_what_runs() {
    let sb = Sandbox::new("narrow");
    sb.install("gitleaks", "", 0, "");
    let off = AnalyzerOptions {
        mode: Mode::Off,
        ..AnalyzerOptions::default()
    };
    assert_eq!(sb.run(&off, None).runs.len(), 0);

    let disabled = AnalyzerOptions {
        disable: vec!["gitleaks".into()],
        ..AnalyzerOptions::default()
    };
    let result = sb.run(&disabled, None);
    assert!(
        status_of(&result, "gitleaks")
            .reason
            .as_ref()
            .unwrap()
            .contains("disabled")
    );
    let missing = status_of(&result, "osv-scanner").reason.clone().unwrap();
    assert!(missing.contains("nothing it analyzes"), "{missing}");

    let entries = inventory(
        &sb.repo,
        &AnalyzerOptions::default(),
        std::slice::from_ref(&sb.bin),
        ADAPTERS,
    );
    let leaks = entries.iter().find(|e| e.id == "gitleaks").unwrap();
    assert!(leaks.installed && leaks.will_run);
    assert_eq!(leaks.version.as_deref(), Some("9.9.9"));
}

#[test]
fn a_tool_that_fails_is_reported_and_the_review_goes_on() {
    let sb = Sandbox::new("fail");
    sb.install(
        "gitleaks",
        "",
        3,
        &format!("echo 'boom at {}/x' >&2", sb.repo.display()),
    );
    let result = sb.run(&AnalyzerOptions::default(), None);
    let run = status_of(&result, "gitleaks");
    assert_eq!(run.status, Status::Failed);
    let reason = run.reason.as_ref().unwrap();
    assert!(
        reason.starts_with("exited with code 3: boom at ./x"),
        "{reason}"
    );
    assert!(
        !reason.contains(&*sb.repo.to_string_lossy()),
        "no repository path: {reason}"
    );

    sb.install("gitleaks", "this is not sarif", 0, "");
    let result = sb.run(&AnalyzerOptions::default(), None);
    assert!(
        status_of(&result, "gitleaks")
            .reason
            .as_ref()
            .unwrap()
            .starts_with("output not understood")
    );
}

#[test]
fn a_tool_that_does_not_stop_is_stopped() {
    let sb = Sandbox::new("slow");
    sb.install("gitleaks", "", 0, "sleep 30");
    let options = AnalyzerOptions {
        timeout_seconds: Some(1),
        ..AnalyzerOptions::default()
    };
    let started = std::time::Instant::now();
    let result = sb.run(&options, None);
    assert!(started.elapsed().as_secs() < 10);
    assert_eq!(
        status_of(&result, "gitleaks").reason.as_deref(),
        Some("timed out after 1s")
    );
}

#[test]
fn the_generated_config_is_removed_and_findings_are_limited_to_the_scope() {
    let sb = Sandbox::new("scope");
    sb.write("config.env", "KEY=value\n");
    sb.install("gitleaks", &fixture("gitleaks.sarif"), 0, "");
    sb.write("other.env", "x\n");

    let args = || std::fs::read_to_string(sb.bin.join("gitleaks.args")).unwrap();
    let all = sb.run(&AnalyzerOptions::default(), None);
    assert_eq!(status_of(&all, "gitleaks").findings, 2);
    let config = args().split("--config ").nth(1).unwrap().trim().to_owned();
    assert!(
        !Path::new(&config).exists(),
        "temporary config left behind: {config}"
    );

    let scoped = sb.run(&AnalyzerOptions::default(), Some(&["other.env".to_owned()]));
    assert_eq!(
        status_of(&scoped, "gitleaks").findings,
        0,
        "config.env is not under review"
    );
}

#[test]
fn analyzer_findings_cite_the_tool_and_a_secret_is_never_excerpted() {
    let sb = Sandbox::new("merge");
    sb.write("config.env", "AWS=AKIAABCDEFGHIJKLMNOP\ntoken=1\n");
    sb.install("gitleaks", &fixture("gitleaks.sarif"), 0, "");
    let result = sb.run(&AnalyzerOptions::default(), None);

    let merged =
        merge_analyzer_findings(Vec::new(), &result.results, &sb.repo, "policy", &FixedEnv)
            .unwrap();
    assert_eq!(merged.len(), 2);
    for finding in &merged {
        let tool = finding.evidence.iter().find_map(|e| match e {
            Evidence::ExternalAnalyzer {
                tool,
                license,
                version,
                ..
            } => Some((tool.as_str(), license.as_str(), version.as_deref())),
            _ => None,
        });
        assert_eq!(tool, Some(("gitleaks", "MIT", Some("9.9.9"))));
        assert!(
            finding
                .evidence
                .iter()
                .any(|e| matches!(e, Evidence::Code { excerpt: None, .. })),
            "a secret line is not copied"
        );
    }
}

fn read(adapter: &Adapter, name: &str) -> Vec<AnalyzerFinding> {
    (adapter.parse)(&fixture(name), Path::new("/repo")).unwrap()
}

fn by_rule<'a>(found: &'a [AnalyzerFinding], rule: &str) -> &'a AnalyzerFinding {
    found.iter().find(|f| f.rule_id == rule).unwrap()
}

#[test]
fn biome_maps_the_rule_group_to_a_category_and_the_level_to_a_severity() {
    let found = read(&BIOME, "biome-report.json");
    let ids: Vec<_> = found.iter().map(|f| f.rule_id.as_str()).collect();
    assert_eq!(
        ids,
        [
            "lint/suspicious/noExplicitAny",
            "lint/style/useConst",
            "lint/correctness/noUnusedVariables",
            "lint/suspicious/noDoubleEquals",
            "lint/security/noGlobalEval"
        ]
    );
    assert_eq!(
        by_rule(&found, "lint/security/noGlobalEval").category,
        "security"
    );
    assert_eq!(
        by_rule(&found, "lint/suspicious/noDoubleEquals").category,
        "reliability"
    );
    assert_eq!(
        by_rule(&found, "lint/style/useConst").category,
        "maintainability"
    );
    let any = by_rule(&found, "lint/suspicious/noExplicitAny");
    assert_eq!(
        (any.severity, any.file.as_str(), any.line, any.column),
        (Severity::Low, "t.ts", Some(3), Some(8))
    );
    assert_eq!(
        any.url.as_deref(),
        Some("https://biomejs.dev/linter/rules/no-explicit-any")
    );
}

#[test]
fn eslint_uses_the_repositorys_severity_and_keeps_the_range() {
    let found = read(&ESLINT, "eslint.json");
    assert_eq!(by_rule(&found, "eqeqeq").severity, Severity::Medium);
    assert_eq!(by_rule(&found, "no-var").severity, Severity::Low);
    let eval = by_rule(&found, "no-eval");
    assert_eq!(
        (eval.category.as_str(), eval.file.as_str()),
        ("security", "app.js")
    );
    assert_eq!(
        eval.url.as_deref(),
        Some("https://eslint.org/docs/latest/rules/no-eval")
    );
    let var = by_rule(&found, "no-var");
    assert_eq!(
        (var.line, var.column, var.end_line, var.end_column),
        (Some(1), Some(1), Some(1), Some(11))
    );
}

#[test]
fn ruff_reads_codes_positions_and_urls() {
    let found = read(&RUFF, "ruff.json");
    let ids: Vec<_> = found.iter().map(|f| f.rule_id.as_str()).collect();
    assert_eq!(ids, ["I001", "F401", "F841"]);
    assert_eq!(
        (found[1].file.as_str(), found[1].line, found[1].column),
        ("app.py", Some(1), Some(12))
    );
    assert!(
        found[1]
            .url
            .as_ref()
            .unwrap()
            .contains("docs.astral.sh/ruff/rules")
    );
    let f401 = by_rule(&found, "F401");
    assert_eq!(
        (f401.category.as_str(), f401.severity),
        ("reliability", Severity::Medium)
    );
    let i001 = by_rule(&found, "I001");
    assert_eq!(
        (i001.category.as_str(), i001.severity),
        ("maintainability", Severity::Low)
    );
}

#[test]
fn ktlint_sarif_after_a_log_line_and_detekt_file_uris() {
    let found = read(&KTLINT, "ktlint.stdout");
    assert_eq!(found.len(), 3);
    let first = &found[0];
    assert_eq!(first.rule_id, "standard:no-wildcard-imports");
    assert_eq!(
        (first.file.as_str(), first.line, first.column),
        ("kt/Main.kt", Some(1), Some(1))
    );
    assert_eq!(
        (first.severity, first.category.as_str()),
        (Severity::Low, "maintainability")
    );

    let found = read(&DETEKT, "detekt.sarif");
    assert_eq!(found.len(), 2);
    let first = &found[0];
    assert_eq!(first.rule_id, "detekt.style.UnusedPrivateProperty");
    assert_eq!(
        (
            first.file.as_str(),
            first.line,
            first.column,
            first.end_column
        ),
        ("kt/Main.kt", Some(3), Some(9), Some(15))
    );
    assert_eq!(
        first.url.as_deref(),
        Some("https://detekt.dev/style.html#unusedprivateproperty")
    );
}

#[test]
fn phpstan_reads_findings_and_never_calls_a_crashed_run_clean() {
    let found = read(&PHPSTAN, "phpstan.json");
    assert_eq!(found.len(), 2);
    assert_eq!(found[0].rule_id, "property.notFound");
    assert_eq!(
        (
            found[0].file.as_str(),
            found[0].line,
            found[0].category.as_str()
        ),
        ("User.php", Some(4), "reliability")
    );
    assert_eq!(
        found[0].url.as_deref(),
        Some("https://phpstan.org/error-identifiers/property.notFound")
    );

    let root = Path::new("/repo");
    assert_eq!(
        (PHPSTAN.parse)(r#"{"tool":"phpstan","result":"passed","errors":0}"#, root)
            .unwrap()
            .len(),
        0
    );
    let failed =
        (PHPSTAN.parse)(r#"{"tool":"phpstan","result":"failed","errors":1}"#, root).unwrap_err();
    assert!(failed.contains("failed run without saying why"));
    let crash = (PHPSTAN.parse)(&fixture("phpstan-crash.json"), root).unwrap_err();
    assert!(
        crash.starts_with("PHPStan could not finish: Child process error"),
        "{crash}"
    );
}

#[test]
fn clippy_reads_lint_codes_and_ignores_build_noise() {
    let found = read(&CLIPPY, "clippy.jsonl");
    let mut ids: Vec<_> = found.iter().map(|f| f.rule_id.as_str()).collect();
    ids.sort_unstable();
    assert_eq!(ids, ["clippy::len_zero", "clippy::useless_vec"]);
    let len = by_rule(&found, "clippy::len_zero");
    assert_eq!(
        (len.file.as_str(), len.line, len.column, len.severity),
        ("src/main.rs", Some(7), Some(8), Severity::Low)
    );
    assert!(len.url.as_ref().unwrap().contains("rust-clippy"));
    assert_eq!(
        by_rule(&found, "clippy::useless_vec").category,
        "performance"
    );
}

#[test]
fn a_tool_that_runs_project_code_needs_a_name_and_gets_its_subcommand() {
    let sb = Sandbox::new("clippy");
    sb.write("Cargo.toml", "[package]\nname = \"x\"\n");
    sb.write("src/main.rs", "fn main() {}\n");
    sb.install("cargo", &fixture("clippy.jsonl"), 101, "");

    let result = sb.run(&AnalyzerOptions::default(), None);
    let run = status_of(&result, "clippy");
    assert_eq!(run.status, Status::Skipped);
    assert!(
        run.reason
            .as_ref()
            .unwrap()
            .contains("runs code the repository controls")
    );

    let result = sb.run(&enabled(&["clippy"]), None);
    assert_eq!(
        status_of(&result, "clippy").status,
        Status::Ran,
        "exit 101 means it compiled and found things"
    );
    let args = std::fs::read_to_string(sb.bin.join("cargo.args")).unwrap();
    assert!(args.starts_with("clippy --message-format=json"), "{args}");
    assert!(args.contains("--target-dir"), "{args}");
    assert!(
        !args.contains(&*sb.repo.to_string_lossy()),
        "build output stays out of the repository"
    );
}

#[test]
fn a_scoped_review_hands_a_tool_only_its_own_files() {
    let sb = Sandbox::new("filter");
    sb.write("biome.json", "{}");
    sb.write("a.ts", "export {}\n");
    sb.write("b.py", "x = 1\n");
    sb.install("biome", r#"{"diagnostics":[]}"#, 0, "");

    let files = ["a.ts".to_owned(), "b.py".to_owned(), "-odd.ts".to_owned()];
    let result = sb.run(&AnalyzerOptions::default(), Some(&files));
    assert_eq!(status_of(&result, "biome").status, Status::Ran);
    let args = std::fs::read_to_string(sb.bin.join("biome.args")).unwrap();
    assert!(
        args.trim_end().ends_with("--max-diagnostics=none a.ts"),
        "{args}"
    );

    let only_python = ["b.py".to_owned()];
    let result = sb.run(&AnalyzerOptions::default(), Some(&only_python));
    assert_eq!(
        status_of(&result, "biome").reason.as_deref(),
        Some("no matching files in this review")
    );
}

#[test]
fn where_a_linter_reports_the_same_rule_on_the_same_line_its_finding_replaces_ours() {
    use debuggatha_core::knowledge_system::ReviewPolicy;
    use debuggatha_engine::detect::{SourceUnit, default_registry, run_review};
    let policy: ReviewPolicy = serde_json::from_value(serde_json::json!({
        "id": "p",
        "rules": [{
            "id": "no-eval", "statement": "Do not evaluate strings as code.",
            "origin": { "kind": "pack", "packId": "debuggatha/owasp", "packVersion": "1.0.0" }
        }],
        "packRefs": [], "conflicts": []
    }))
    .unwrap();
    let unit = SourceUnit {
        file: "app.js".into(),
        content: "const y = 1;\neval(x);\neval(z);\n".into(),
        changed_lines: None,
    };
    let ours = run_review(&policy, &[unit], &default_registry(), &FixedEnv)
        .unwrap()
        .findings;
    assert_eq!(ours.len(), 2);

    let sb = Sandbox::new("equiv");
    sb.write("eslint.config.js", "export default [];\n");
    sb.write("app.js", "const y = 1;\neval(x);\neval(z);\n");
    sb.install(
        "eslint",
        &fixture("eslint.json").replace("/repo", &sb.repo.to_string_lossy()),
        1,
        "",
    );
    let result = sb.run(&enabled(&["eslint"]), None);
    let merged = merge_analyzer_findings(ours, &result.results, &sb.repo, "p", &FixedEnv).unwrap();
    let internal = merged
        .iter()
        .filter(|f| {
            !f.evidence
                .iter()
                .any(|e| matches!(e, Evidence::ExternalAnalyzer { .. }))
        })
        .count();
    let eslint_eval = result.results[0]
        .findings
        .iter()
        .filter(|f| f.rule_id == "no-eval")
        .count();
    assert_eq!(
        eslint_eval, 1,
        "the recorded ESLint output reports one no-eval"
    );
    assert_eq!(
        internal, 1,
        "the no-eval ESLint reported on the same line is not said twice"
    );
}
