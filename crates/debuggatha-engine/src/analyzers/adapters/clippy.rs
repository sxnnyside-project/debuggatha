//! Clippy: Rust lint.

use super::shared::{Draft, Position, located, num, strings};
use crate::analyzers::types::{Adapter, AnalyzerFinding, Invocation, RunContext, Trust};
use debuggatha_core::findings_ledger::sha256_hex;
use debuggatha_core::review_engine::Severity;
use serde_json::Value;
use std::path::Path;

fn clippy_category(rule: &str) -> &'static str {
    const RELIABILITY: &[&str] = &[
        "unwrap_used",
        "expect_used",
        "panic",
        "indexing_slicing",
        "unreachable",
        "todo",
        "unimplemented",
    ];
    if RELIABILITY.iter().any(|p| rule.contains(p)) {
        "reliability"
    } else if [
        "clippy::perf",
        "clippy::redundant_clone",
        "clippy::useless_vec",
        "clippy::needless_collect",
    ]
    .iter()
    .any(|p| rule.starts_with(p))
    {
        "performance"
    } else {
        "maintainability"
    }
}

#[allow(clippy::unnecessary_wraps)] // the shape every adapter's `parse` has
fn clippy_parse(stdout: &str, root: &Path) -> Result<Vec<AnalyzerFinding>, String> {
    Ok(stdout
        .lines()
        .filter(|l| l.starts_with('{'))
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .filter_map(|line| {
            let message = &line["message"];
            let rule = message["code"]["code"].as_str().filter(|c| !c.is_empty())?;
            if line["reason"].as_str() != Some("compiler-message") {
                return None;
            }
            let level = message["level"].as_str()?;
            if !matches!(level, "warning" | "error") {
                return None;
            }
            let spans = message["spans"].as_array()?;
            let span = spans
                .iter()
                .find(|s| s["is_primary"].as_bool() == Some(true))
                .or_else(|| spans.first())?;
            let help: String = message["children"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|c| c["message"].as_str())
                .collect::<Vec<_>>()
                .join(" ");
            located(
                Draft {
                    rule_id: rule.to_owned(),
                    message: message["message"].as_str().unwrap_or(rule).to_owned(),
                    severity: if level == "error" {
                        Severity::High
                    } else {
                        Severity::Low
                    },
                    category: clippy_category(rule),
                    url: documentation_link(&help),
                },
                root,
                span["file_name"].as_str()?,
                Position {
                    line: num(&span["line_start"]),
                    column: num(&span["column_start"]),
                    end_line: num(&span["line_end"]),
                    end_column: num(&span["column_end"]),
                },
            )
        })
        .collect())
}

/// The first `https://rust-lang.github.io/...` link in Clippy's help text.
fn documentation_link(help: &str) -> Option<String> {
    let start = help.find("https://rust-lang.github.io/")?;
    let link: String = help[start..]
        .chars()
        .take_while(|c| !c.is_whitespace() && *c != '`')
        .collect();
    Some(link)
}

fn clippy_command(context: &RunContext<'_>) -> Invocation {
    // Build output goes to the temp directory, never into the repository's `target/`.
    let key = sha256_hex(&context.root.to_string_lossy());
    let target = std::env::temp_dir().join(format!("debuggatha-clippy-{}", &key[..12]));
    let mut args = strings(&[
        "--message-format=json",
        "--quiet",
        "--workspace",
        "--all-targets",
    ]);
    args.extend([
        "--target-dir".to_owned(),
        target.to_string_lossy().into_owned(),
    ]);
    Invocation {
        args,
        cleanup: None,
    }
}

/// Clippy, the Rust linter; it compiles the project, so it runs project code.
pub static CLIPPY: Adapter = Adapter {
    id: "clippy",
    name: "Clippy",
    license: "MIT OR Apache-2.0",
    homepage: "https://doc.rust-lang.org/clippy",
    covers: "Rust",
    // Clippy compiles the crate: build scripts and procedural macros run, and missing dependencies
    // are downloaded.
    trust: Trust::RunsProjectCode,
    applies: |facts| facts.has_file_named(&["Cargo.toml"]),
    local_binaries: &[],
    binary_names: &["cargo"],
    prefix: &["clippy"],
    version_args: &["--version"],
    command: clippy_command,
    ok_exit_codes: &[0, 101],
    parse: clippy_parse,
    supersedes: &[],
    equivalent_to: &[],
    file_filter: None,
};
