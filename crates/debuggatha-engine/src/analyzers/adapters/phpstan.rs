//! `PHPStan`: PHP static analysis.

use super::shared::{Draft, Position, extension, located, num, strings};
use crate::analyzers::sarif::json_from;
use crate::analyzers::types::{Adapter, AnalyzerFinding, Invocation, RepoFacts, RunContext, Trust};
use debuggatha_core::review_engine::Severity;
use serde_json::Value;
use std::path::Path;

const PHPSTAN_CONFIGS: &[&str] = &["phpstan.neon", "phpstan.neon.dist", "phpstan.dist.neon"];
const PHP_SOURCE_DIRS: &[&str] = &["src", "app", "lib"];

fn phpstan_command(context: &RunContext<'_>) -> Invocation {
    let configured = PHPSTAN_CONFIGS
        .iter()
        .any(|name| context.root.join(name).exists());
    let directories: Vec<String> = PHP_SOURCE_DIRS
        .iter()
        .filter(|name| context.root.join(name).exists())
        .map(|name| (*name).to_owned())
        .collect();
    let mut args = strings(&[
        "analyse",
        "--error-format=json",
        "--no-progress",
        "--no-interaction",
        // PHP's own default of 128M is too little for a real application.
        "--memory-limit=1G",
    ]);
    // Without a config there is no level; 0 only reports what is certainly wrong.
    if !configured {
        args.push("--level=0".to_owned());
    }
    match context.files {
        Some(files) => args.extend(files.iter().cloned()),
        None if configured => {}
        None if directories.is_empty() => args.push(".".to_owned()),
        None => args.extend(directories),
    }
    Invocation {
        args,
        cleanup: None,
    }
}

fn phpstan_parse(stdout: &str, root: &Path) -> Result<Vec<AnalyzerFinding>, String> {
    let report = json_from(stdout)?;
    // A run that did not finish says nothing about the code; "no findings" would be a lie.
    let problems = report["errors"]
        .as_array()
        .into_iter()
        .chain(report["general_errors"].as_array())
        .flatten()
        .find_map(Value::as_str);
    if let Some(problem) = problems {
        let first = problem.lines().next().unwrap_or("");
        return Err(format!("PHPStan could not finish: {first}"));
    }
    if report["result"].as_str() == Some("failed") && report.get("files").is_none() {
        return Err("PHPStan reported a failed run without saying why".to_owned());
    }
    Ok(report["files"]
        .as_object()
        .into_iter()
        .flatten()
        .flat_map(|(file, entry)| {
            entry["messages"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(move |m| {
                    let identifier = m["identifier"].as_str();
                    located(
                        Draft {
                            rule_id: identifier.unwrap_or("phpstan").to_owned(),
                            message: m["message"].as_str().unwrap_or("").to_owned(),
                            severity: Severity::Medium,
                            category: "reliability",
                            url: identifier
                                .map(|i| format!("https://phpstan.org/error-identifiers/{i}")),
                        },
                        root,
                        file,
                        Position {
                            line: num(&m["line"]).filter(|l| *l > 0),
                            ..Position::default()
                        },
                    )
                })
        })
        .collect())
}

fn phpstan_applies(facts: &RepoFacts) -> bool {
    facts.any(|f| extension(f) == "php")
        && facts.has_file_named(&[
            "composer.json",
            "phpstan.neon",
            "phpstan.dist.neon",
            "phpstan.neon.dist",
            "phpstan.dist.neon.dist",
        ])
}

pub static PHPSTAN: Adapter = Adapter {
    id: "phpstan",
    name: "PHPStan",
    license: "MIT",
    homepage: "https://phpstan.org",
    covers: "PHP type and logic errors",
    // PHPStan loads the project's autoloader and bootstrap files.
    trust: Trust::RunsProjectCode,
    applies: phpstan_applies,
    local_binaries: &["vendor/bin/phpstan"],
    binary_names: &["phpstan"],
    prefix: &[],
    version_args: &["--version"],
    command: phpstan_command,
    ok_exit_codes: &[0, 1],
    parse: phpstan_parse,
    supersedes: &[],
    equivalent_to: &[],
    file_filter: Some(|f| extension(f) == "php"),
};
