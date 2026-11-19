//! Ruff: Python lint.

use super::shared::{Draft, Position, extension, located, num, scoped, strings};
use crate::analyzers::types::{Adapter, AnalyzerFinding, Invocation, RunContext, Trust};
use debuggatha_core::review_engine::Severity;
use serde_json::Value;
use std::path::Path;

fn ruff_parse(stdout: &str, root: &Path) -> Result<Vec<AnalyzerFinding>, String> {
    let diagnostics: Value = serde_json::from_str(stdout).map_err(|e| e.to_string())?;
    Ok(diagnostics
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|d| {
            let original = d["code"].as_str();
            let code = original.unwrap_or("syntax-error");
            let flaw = ["F", "E9", "B"].iter().any(|p| code.starts_with(p));
            let security = code.starts_with('S');
            located(
                Draft {
                    rule_id: code.to_owned(),
                    message: d["message"].as_str().unwrap_or(code).to_owned(),
                    severity: if flaw || security || original.is_none() {
                        Severity::Medium
                    } else {
                        Severity::Low
                    },
                    category: if security {
                        "security"
                    } else if flaw || original.is_none() {
                        "reliability"
                    } else {
                        "maintainability"
                    },
                    url: d["url"].as_str().map(str::to_owned),
                },
                root,
                d["filename"].as_str()?,
                Position {
                    line: num(&d["location"]["row"]),
                    column: num(&d["location"]["column"]),
                    end_line: num(&d["end_location"]["row"]),
                    end_column: num(&d["end_location"]["column"]),
                },
            )
        })
        .collect())
}

fn ruff_command(context: &RunContext<'_>) -> Invocation {
    // `--no-cache` keeps Ruff from writing `.ruff_cache` into the repository.
    let mut args = strings(&[
        "check",
        "--output-format",
        "json",
        "--exit-zero",
        "--no-cache",
    ]);
    args.extend(scoped(context, &["."]));
    Invocation {
        args,
        cleanup: None,
    }
}

/// Ruff, the Python linter.
pub static RUFF: Adapter = Adapter {
    id: "ruff",
    name: "Ruff",
    license: "MIT",
    homepage: "https://docs.astral.sh/ruff",
    covers: "Python",
    trust: Trust::Safe,
    applies: |facts| facts.any(|f| matches!(extension(f), "py" | "pyi")),
    local_binaries: &[".venv/bin/ruff", "venv/bin/ruff"],
    binary_names: &["ruff"],
    prefix: &[],
    version_args: &["--version"],
    command: ruff_command,
    ok_exit_codes: &[0],
    parse: ruff_parse,
    supersedes: &[],
    equivalent_to: &[],
    file_filter: Some(|f| matches!(extension(f), "py" | "pyi")),
};
