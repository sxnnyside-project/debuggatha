//! The machine and review-comment formats of a review: GitHub annotations, GitLab Code Quality,
//! a pull-request comment in Markdown, and SARIF.

use debuggatha_engine::core::findings_ledger::sha256_hex;
use debuggatha_engine::core::review_engine::{
    Evidence, Finding, SemanticRole, Severity, rule_id_from_evidence,
};
use serde_json::{Value, json};

/// The rule a finding traces back to: the pack rule or criterion named in its evidence.
pub(crate) fn rule_id_of(finding: &Finding) -> String {
    rule_id_from_evidence(&finding.evidence).unwrap_or_else(|| format!("{:?}", finding.category))
}

pub(crate) fn severity_name(severity: Severity) -> &'static str {
    match severity {
        Severity::Critical => "critical",
        Severity::High => "high",
        Severity::Medium => "medium",
        Severity::Low => "low",
        Severity::Informational => "informational",
    }
}

struct Where<'a> {
    file: &'a str,
    start: Option<u32>,
    end: Option<u32>,
    column: Option<u32>,
    end_column: Option<u32>,
}

fn location(finding: &Finding) -> Where<'_> {
    let first = finding.locations.first();
    Where {
        file: first.map_or("", |l| l.file.as_str()),
        start: first.and_then(|l| l.lines).map(|r| r.start),
        end: first.and_then(|l| l.lines).map(|r| r.end),
        column: first.and_then(|l| l.columns).map(|r| r.start),
        end_column: first.and_then(|l| l.columns).map(|r| r.end),
    }
}

pub(crate) fn analyzer_of(finding: &Finding) -> Option<(&str, Option<&str>, &str)> {
    finding.evidence.iter().find_map(|e| match e {
        Evidence::ExternalAnalyzer {
            tool,
            version,
            license,
            ..
        } => Some((tool.as_str(), version.as_deref(), license.as_str())),
        _ => None,
    })
}

fn is_model_suspicion(finding: &Finding) -> bool {
    finding.evidence.iter().any(|e| {
        matches!(
            e,
            Evidence::SemanticReview {
                role: SemanticRole::Detection,
                ..
            }
        )
    })
}

fn by_severity(findings: &[Finding]) -> Vec<&Finding> {
    let mut sorted: Vec<&Finding> = findings.iter().collect();
    sorted.sort_by_key(|f| f.severity);
    sorted
}

fn escape_data(value: &str) -> String {
    value
        .replace('%', "%25")
        .replace('\r', "%0D")
        .replace('\n', "%0A")
}

fn escape_property(value: &str) -> String {
    escape_data(value).replace(':', "%3A").replace(',', "%2C")
}

/// GitHub Actions workflow commands: each finding becomes an inline annotation.
pub(crate) fn github(findings: &[Finding]) -> String {
    findings
        .iter()
        .map(|f| {
            let at = location(f);
            let level = match f.severity {
                Severity::Critical | Severity::High => "error",
                Severity::Medium => "warning",
                Severity::Low | Severity::Informational => "notice",
            };
            let mut props = vec![format!("file={}", escape_property(at.file))];
            if let Some(start) = at.start {
                props.push(format!("line={start}"));
                props.push(format!("endLine={}", at.end.unwrap_or(start)));
            }
            if let (Some(c), Some(e)) = (at.column, at.end_column) {
                props.push(format!("col={c}"));
                props.push(format!("endColumn={e}"));
            }
            props.push(format!(
                "title={}",
                escape_property(&format!("{} ({})", f.title, rule_id_of(f)))
            ));
            format!(
                "::{level} {}::{}",
                props.join(","),
                escape_data(&f.explanation)
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// GitLab's Code Quality report. The fingerprint comes from what identifies a finding, never from
/// a code excerpt.
pub(crate) fn gitlab(findings: &[Finding]) -> String {
    let list: Vec<Value> = findings
        .iter()
        .map(|f| {
            let at = location(f);
            let rule = rule_id_of(f);
            let identity = [
                rule.as_str(),
                at.file,
                &at.start.map(|s| s.to_string()).unwrap_or_default(),
                &f.title,
            ]
            .join("\0");
            json!({
                "description": f.title,
                "check_name": rule,
                "fingerprint": &sha256_hex(&identity)[..32],
                "severity": match f.severity {
                    Severity::Informational => "info",
                    Severity::Low => "minor",
                    Severity::Medium => "major",
                    Severity::High => "critical",
                    Severity::Critical => "blocker",
                },
                "location": { "path": at.file.replace('\\', "/"), "lines": { "begin": at.start.unwrap_or(1) } },
            })
        })
        .collect();
    serde_json::to_string_pretty(&list).unwrap_or_default()
}

/// Marks a comment as this report's, so a CI job can update it in place instead of adding another.
pub(crate) const MARKDOWN_MARKER: &str = "<!-- debuggatha-report -->";
const MARKDOWN_ROWS: usize = 40;

fn cell(value: &str) -> String {
    value
        .replace("\r\n", " ")
        .replace(['\n', '\r'], " ")
        .replace('|', "\\|")
        .replace('<', "&lt;")
}

/// Where a finding is on GitHub, when the run knows the repository and commit (Actions sets both).
fn blob_link(file: &str, line: Option<u32>) -> Option<String> {
    let server = std::env::var("GITHUB_SERVER_URL").ok()?;
    let repo = std::env::var("GITHUB_REPOSITORY").ok()?;
    let sha = std::env::var("GITHUB_SHA").ok()?;
    let path = file
        .split('/')
        .map(|s| {
            s.replace('%', "%25")
                .replace(' ', "%20")
                .replace('#', "%23")
        })
        .collect::<Vec<_>>()
        .join("/");
    let anchor = line.map_or_else(String::new, |l| format!("#L{l}"));
    Some(format!("{server}/{repo}/blob/{sha}/{path}{anchor}"))
}

/// A pull-request comment: the counts first, then what to look at, in a table a person can scan.
pub(crate) fn markdown(findings: &[Finding], notes: &[String], uncounted: Option<&str>) -> String {
    let sorted = by_severity(findings);
    let mut out = vec![
        MARKDOWN_MARKER.to_owned(),
        "## Debuggatha review".to_owned(),
        String::new(),
    ];
    if findings.is_empty() {
        out.push("No findings.".to_owned());
    } else {
        let counts: Vec<String> = Severity::ALL
            .iter()
            .filter_map(|s| {
                let n = findings.iter().filter(|f| f.severity == *s).count();
                (n > 0).then(|| format!("{n} {}", severity_name(*s)))
            })
            .collect();
        out.push(format!(
            "**{} {}**: {}",
            findings.len(),
            if findings.len() == 1 {
                "finding"
            } else {
                "findings"
            },
            counts.join(", ")
        ));
        out.push(String::new());
        out.push("| Severity | Where | Finding | Rule |".to_owned());
        out.push("| --- | --- | --- | --- |".to_owned());
        for f in sorted.iter().take(MARKDOWN_ROWS) {
            let at = location(f);
            let place = format!(
                "{}{}",
                at.file,
                at.start.map_or_else(String::new, |s| format!(":{s}"))
            );
            let shown = match blob_link(at.file, at.start) {
                Some(link) => format!("[`{}`]({link})", cell(&place)),
                None => format!("`{}`", cell(&place)),
            };
            let tool = analyzer_of(f).map_or_else(String::new, |(t, _, l)| format!(" ({t}, {l})"));
            let model = if is_model_suspicion(f) {
                " (model suspicion)"
            } else {
                ""
            };
            out.push(format!(
                "| {} | {shown} | {} | {} |",
                severity_name(f.severity),
                cell(&f.title),
                cell(&format!("{}{tool}{model}", rule_id_of(f)))
            ));
        }
        if sorted.len() > MARKDOWN_ROWS {
            out.push(String::new());
            out.push(format!(
                "…and {} more; `debuggatha review` lists them all.",
                sorted.len() - MARKDOWN_ROWS
            ));
        }
        let fixes: Vec<String> = sorted
            .iter()
            .take(MARKDOWN_ROWS)
            .filter_map(|f| {
                let summary = &f.recommendations.first()?.summary;
                let at = location(f);
                Some(format!(
                    "- `{}{}`: {}",
                    cell(at.file),
                    at.start.map_or_else(String::new, |s| format!(":{s}")),
                    cell(summary)
                ))
            })
            .collect();
        if !fixes.is_empty() {
            out.push(String::new());
            out.push("<details><summary>How to fix</summary>".to_owned());
            out.push(String::new());
            out.extend(fixes);
            out.push(String::new());
            out.push("</details>".to_owned());
        }
    }
    if let Some(text) = uncounted {
        out.push(String::new());
        out.push(text.to_owned());
    }
    for note in notes {
        out.push(String::new());
        out.push(format!("_{}_", cell(note)));
    }
    out.join("\n")
}

/// SARIF 2.1.0, the format GitHub code scanning and most CI security dashboards ingest.
pub(crate) fn sarif(findings: &[Finding]) -> String {
    let mut rules: Vec<Value> = Vec::new();
    let mut seen: Vec<String> = Vec::new();
    let results: Vec<Value> = findings
        .iter()
        .map(|f| {
            let rule = rule_id_of(f);
            if !seen.contains(&rule) {
                seen.push(rule.clone());
                let help = f.recommendations.first().map(|r| {
                    let example = r.example.as_ref().map_or_else(String::new, |e| {
                        format!("\n\nBefore:\n{}\n\nAfter:\n{}", e.before, e.after)
                    });
                    json!({ "text": format!("{}{example}", r.summary) })
                });
                let mut entry = json!({
                    "id": rule, "name": rule,
                    "shortDescription": { "text": f.title },
                    "properties": { "precision": format!("{:?}", f.confidence).to_lowercase() },
                });
                if let Some(help) = help {
                    entry["help"] = help;
                }
                rules.push(entry);
            }
            let at = location(f);
            let mut physical = json!({
                "artifactLocation": { "uri": at.file.replace('\\', "/"), "uriBaseId": "%SRCROOT%" },
            });
            if let Some(start) = at.start {
                let mut region = json!({ "startLine": start, "endLine": at.end.unwrap_or(start) });
                if let (Some(c), Some(e)) = (at.column, at.end_column) {
                    region["startColumn"] = json!(c);
                    region["endColumn"] = json!(e);
                }
                physical["region"] = region;
            }
            let mut properties = json!({
                "severity": severity_name(f.severity),
                "confidence": format!("{:?}", f.confidence).to_lowercase(),
                "category": format!("{:?}", f.category),
            });
            for e in &f.evidence {
                match e {
                    Evidence::ExternalAnalyzer {
                        tool,
                        rule_id,
                        version,
                        license,
                        url,
                    } => {
                        properties["analyzer"] = json!({
                            "tool": tool, "ruleId": rule_id, "license": license,
                            "version": version, "url": url,
                        });
                    }
                    Evidence::SemanticReview {
                        role,
                        provider,
                        model,
                        verdict,
                        reason,
                        ..
                    } => {
                        properties["semantic"] = json!({
                            "role": role, "provider": provider, "model": model,
                            "verdict": verdict, "reason": reason,
                        });
                    }
                    _ => {}
                }
            }
            json!({
                "ruleId": rule,
                "level": match f.severity {
                    Severity::Critical | Severity::High => "error",
                    Severity::Medium => "warning",
                    Severity::Low | Severity::Informational => "note",
                },
                "message": { "text": format!("{}. {}", f.title, f.explanation) },
                "locations": [{ "physicalLocation": physical }],
                "properties": properties,
            })
        })
        .collect();
    serde_json::to_string_pretty(&json!({
        "$schema": "https://json.schemastore.org/sarif-2.1.0.json",
        "version": "2.1.0",
        "runs": [{
            "tool": { "driver": {
                "name": "Debuggatha",
                "version": debuggatha_engine::VERSION,
                "informationUri": "https://github.com/sxnnyside-project/debuggatha",
                "rules": rules,
            } },
            "results": results,
        }],
    }))
    .unwrap_or_default()
}
