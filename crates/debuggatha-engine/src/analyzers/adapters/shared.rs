//! What every adapter needs to turn a tool's output into findings.

use crate::analyzers::paths::to_repository_path;
use crate::analyzers::types::{AnalyzerFinding, RunContext};
use debuggatha_core::review_engine::Severity;
use serde_json::Value;
use std::path::Path;

/// What a tool says about one finding before its file is checked against the repository.
pub(super) struct Draft {
    pub(super) rule_id: String,
    pub(super) message: String,
    pub(super) severity: Severity,
    pub(super) category: &'static str,
    pub(super) url: Option<String>,
}

#[derive(Default, Clone, Copy)]
pub(super) struct Position {
    pub(super) line: Option<u32>,
    pub(super) column: Option<u32>,
    pub(super) end_line: Option<u32>,
    pub(super) end_column: Option<u32>,
}

/// `None` when the file is outside the repository.
pub(super) fn located(
    draft: Draft,
    root: &Path,
    file: &str,
    at: Position,
) -> Option<AnalyzerFinding> {
    Some(AnalyzerFinding {
        rule_id: draft.rule_id,
        message: draft.message,
        severity: draft.severity,
        category: draft.category.to_owned(),
        file: to_repository_path(root, file)?,
        line: at.line,
        column: at.column,
        end_line: at.end_line,
        end_column: at.end_column,
        url: draft.url,
        sensitive: false,
    })
}

pub(super) fn num(value: &Value) -> Option<u32> {
    value.as_u64().and_then(|n| u32::try_from(n).ok())
}

pub(super) fn scoped(context: &RunContext<'_>, fallback: &[&str]) -> Vec<String> {
    context.files.map_or_else(
        || fallback.iter().map(|s| (*s).to_owned()).collect(),
        <[String]>::to_vec,
    )
}

pub(super) fn strings(args: &[&str]) -> Vec<String> {
    args.iter().map(|a| (*a).to_owned()).collect()
}

pub(super) fn first_prefix(
    table: &[(&[&str], &'static str)],
    id: &str,
    fallback: &'static str,
) -> &'static str {
    table
        .iter()
        .find(|(prefixes, _)| prefixes.iter().any(|p| id.starts_with(p)))
        .map_or(fallback, |(_, category)| category)
}

pub(super) fn extension(file: &str) -> &str {
    file.rsplit_once('.').map_or("", |(_, e)| e)
}

pub(super) fn is_script(file: &str) -> bool {
    // `.js .jsx .ts .tsx`, each with an optional `m` or `c` before it.
    let ext = extension(file);
    let ext = ext.strip_prefix(['m', 'c']).unwrap_or(ext);
    matches!(ext, "js" | "jsx" | "ts" | "tsx")
}

pub(super) fn base_name(file: &str) -> &str {
    file.rsplit('/').next().unwrap_or(file)
}
