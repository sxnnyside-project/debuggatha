use std::process::{Command, Stdio};

use debuggatha_engine::analyzers::{ADAPTERS, AnalyzerOptions, inventory, path_dirs};
use debuggatha_engine::baseline::load_baseline;
use debuggatha_engine::core::env::SystemEnv;
use debuggatha_engine::core::findings_ledger::load_ledger;
use debuggatha_engine::core::repository_intelligence::build_repository_context;
use debuggatha_engine::core::repository_memory::load_memory_store;
use serde_json::json;

use crate::report::Format;
use crate::{Context, client_message};

struct Check {
    name: &'static str,
    /// `None` is a note, not a pass or a failure.
    ok: Option<bool>,
    detail: String,
}

fn check(name: &'static str, ok: Option<bool>, detail: impl Into<String>) -> Check {
    Check {
        name,
        ok,
        detail: detail.into(),
    }
}

/// Verify the repository and the environment: what a review needs, and what it would refuse.
pub(crate) fn run(context: &Context) -> Result<bool, String> {
    let root = context.root()?;
    let env = &SystemEnv;
    let mut checks = vec![check(
        "version",
        Some(true),
        format!("debuggatha {}", debuggatha_engine::VERSION),
    )];

    let git = Command::new("git")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success());
    checks.push(if git {
        check("git", Some(true), "git is installed.")
    } else {
        check(
            "git",
            None,
            "git is not installed; reviewing changes needs it.",
        )
    });
    checks.push(if root.join(".git").exists() {
        check("repository", Some(true), "Git repository detected.")
    } else {
        check(
            "repository",
            None,
            "No .git directory; reviews cover the whole repository.",
        )
    });
    checks.push(if root.join(".debuggatha").exists() {
        check("state", Some(true), ".debuggatha directory found.")
    } else {
        check(
            "state",
            None,
            "No .debuggatha directory yet; the first persisted review creates it.",
        )
    });

    checks.push(match build_repository_context(&root, env, None) {
        Ok(_) => check("context", Some(true), "Repository context built."),
        Err(e) => check("context", Some(false), client_message(&e)),
    });
    checks.push(stored("ledger", load_ledger(&root, env).map(drop)));
    checks.push(stored("memory", load_memory_store(&root, env).map(drop)));
    checks.push(stored("baseline", load_baseline(&root).map(drop)));

    let found = inventory(&root, &AnalyzerOptions::default(), &path_dirs(), ADAPTERS);
    let usable = found.iter().filter(|e| e.applies && e.installed).count();
    checks.push(check(
        "analyzers",
        None,
        format!("{usable} analyzer(s) installed and used by this repository."),
    ));

    let healthy = checks.iter().all(|c| c.ok != Some(false));
    match context.format() {
        Format::Json => {
            let list: Vec<_> = checks
                .iter()
                .map(|c| json!({ "name": c.name, "ok": c.ok, "detail": c.detail }))
                .collect();
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({
                    "status": if healthy { "success" } else { "error" },
                    "checks": list,
                }))
                .unwrap_or_default()
            );
        }
        Format::Text => {
            for c in &checks {
                let mark = match c.ok {
                    Some(true) => "ok  ",
                    Some(false) => "FAIL",
                    None => "note",
                };
                println!("{mark} {}: {}", c.name, c.detail);
            }
        }
    }
    if healthy {
        Ok(false)
    } else {
        Err("Diagnostics failed.".to_owned())
    }
}

fn stored<E: std::error::Error + 'static>(name: &'static str, loaded: Result<(), E>) -> Check {
    match loaded {
        Ok(()) => check(
            name,
            Some(true),
            format!("The {name} file is absent or valid."),
        ),
        Err(e) => check(name, Some(false), client_message(&e)),
    }
}
