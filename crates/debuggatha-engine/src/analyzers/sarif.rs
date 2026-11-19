//! Reads the SARIF output many analyzers print.

use std::path::Path;

use debuggatha_core::review_engine::Severity;
use serde_json::Value;

use super::paths::to_repository_path;
use super::types::AnalyzerFinding;

/// How to read a tool's SARIF output into findings.
pub struct SarifOptions {
    /// Maps the tool's rule id to a finding category.
    pub category: fn(&str) -> &'static str,
    /// Severity from SARIF's `level` when the tool gives none of its own; `None` maps the level
    /// with [`severity_from_level`].
    pub severity: Option<fn(Option<&str>) -> Severity>,
    /// The tool's findings may include secrets, so their excerpts are never copied.
    pub sensitive: bool,
}

/// Tools print logs beside their output, so the document is whatever starts at the first line
/// that begins with `{`.
///
/// # Errors
/// There is no JSON document in the text, or it does not parse.
pub fn json_from(text: &str) -> Result<Value, String> {
    let start = if text.starts_with('{') {
        Some(0)
    } else {
        text.find("\n{").map(|i| i + 1)
    }
    .or_else(|| text.find('{'))
    .ok_or("no JSON document in the output")?;
    serde_json::from_str(&text[start..]).map_err(|e| e.to_string())
}

/// SARIF `level` onto the severity rubric; a style tool's `error` is a rule it enforces, not a
/// defect.
#[must_use]
pub fn severity_from_level(level: Option<&str>) -> Severity {
    match level {
        Some("error") => Severity::Medium,
        Some("note" | "none") => Severity::Informational,
        _ => Severity::Low,
    }
}

/// CVSS-style scores (`security-severity`) onto the rubric.
fn severity_from_score(score: f64) -> Severity {
    if score >= 9.0 {
        Severity::Critical
    } else if score >= 7.0 {
        Severity::High
    } else if score >= 4.0 {
        Severity::Medium
    } else {
        Severity::Low
    }
}

fn number(value: Option<&Value>) -> Option<u32> {
    value?.as_u64().and_then(|n| u32::try_from(n).ok())
}

/// Reads any SARIF 2.1.0 document; the adapters only decide category and severity.
///
/// # Errors
/// The text holds no JSON document.
pub fn parse_sarif(
    text: &str,
    root: &Path,
    options: &SarifOptions,
) -> Result<Vec<AnalyzerFinding>, String> {
    let document = json_from(text)?;
    let mut findings = Vec::new();
    for run in document["runs"].as_array().into_iter().flatten() {
        let rules: Vec<&Value> = run["tool"]["driver"]["rules"]
            .as_array()
            .into_iter()
            .flatten()
            .collect();
        for result in run["results"].as_array().into_iter().flatten() {
            let location = &result["locations"][0]["physicalLocation"];
            let Some(file) = location["artifactLocation"]["uri"]
                .as_str()
                .and_then(|uri| to_repository_path(root, uri))
            else {
                continue;
            };
            let rule_id = result["ruleId"].as_str().unwrap_or("unknown");
            let rule = rules.iter().find(|r| r["id"].as_str() == Some(rule_id));
            let score = rule.and_then(|r| {
                let s = &r["properties"]["security-severity"];
                s.as_f64().or_else(|| s.as_str()?.parse().ok())
            });
            let level = result["level"].as_str();
            let severity = match (score, options.severity) {
                (Some(score), _) => severity_from_score(score),
                (None, Some(from_level)) => from_level(level),
                (None, None) => severity_from_level(level),
            };
            let region = &location["region"];
            findings.push(AnalyzerFinding {
                rule_id: rule_id.to_owned(),
                message: result["message"]["text"]
                    .as_str()
                    .unwrap_or(rule_id)
                    .to_owned(),
                severity,
                category: (options.category)(rule_id).to_owned(),
                file,
                line: number(region.get("startLine")),
                column: number(region.get("startColumn")),
                end_line: number(region.get("endLine")),
                end_column: number(region.get("endColumn")),
                url: rule
                    .and_then(|r| r["helpUri"].as_str())
                    .filter(|u| !u.is_empty())
                    .map(str::to_owned),
                sensitive: options.sensitive,
            });
        }
    }
    Ok(findings)
}
