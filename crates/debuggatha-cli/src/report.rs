use debuggatha_engine::analyzers::{AnalyzerRun, Status};
use debuggatha_engine::core::review_engine::{
    Evidence, Finding, ReviewSummary, SemanticRole, SemanticVerdict,
};
use debuggatha_engine::pipeline::ReviewOutput;
use serde_json::json;

use crate::formats::{self, analyzer_of, rule_id_of, severity_name};

/// What every command but `review` can print.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Format {
    Text,
    Json,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub(crate) enum ReportFormat {
    Text,
    Json,
    Sarif,
    Github,
    Markdown,
    Gitlab,
}

impl ReportFormat {
    pub(crate) fn parse(name: &str) -> Option<Self> {
        <Self as clap::ValueEnum>::from_str(name, true).ok()
    }
}

/// A review as it is reported: the findings after the config's ignore list, and what to say
/// besides them.
pub(crate) struct View<'a> {
    pub(crate) output: &'a ReviewOutput,
    pub(crate) findings: &'a [Finding],
    pub(crate) summary: &'a ReviewSummary,
    /// How many findings count toward the exit code.
    pub(crate) counted: usize,
    pub(crate) notes: Vec<String>,
    pub(crate) uncounted: Option<String>,
    pub(crate) ignored: usize,
    pub(crate) quiet: bool,
    pub(crate) persisted: bool,
}

pub(crate) fn render(view: &View<'_>, format: ReportFormat) -> String {
    match format {
        ReportFormat::Json => json_report(view),
        ReportFormat::Sarif => formats::sarif(view.findings),
        ReportFormat::Github => formats::github(view.findings),
        ReportFormat::Markdown => {
            formats::markdown(view.findings, &view.notes, view.uncounted.as_deref())
        }
        ReportFormat::Gitlab => formats::gitlab(view.findings),
        ReportFormat::Text => text(view),
    }
}

/// What a machine format says when nothing changed, so a CI step always has a file to read.
pub(crate) fn empty(format: ReportFormat, note: &str) -> String {
    match format {
        ReportFormat::Json => serde_json::to_string_pretty(&json!({
            "status": "success", "findings": [], "note": note, "analyzers": [], "semantic": null,
        }))
        .unwrap_or_default(),
        ReportFormat::Sarif => formats::sarif(&[]),
        ReportFormat::Markdown => formats::markdown(&[], &[note.to_owned()], None),
        ReportFormat::Gitlab => "[]".to_owned(),
        ReportFormat::Text | ReportFormat::Github => String::new(),
    }
}

fn json_report(view: &View<'_>) -> String {
    let o = view.output;
    serde_json::to_string_pretty(&json!({
        "status": "success",
        "policyId": o.policy.id,
        "summary": view.summary,
        "findings": view.findings,
        "baselined": o.baselined,
        "suppressedByMemory": o.suppressed_by_memory.len(),
        "suppressedInline": o.suppressed_inline.len(),
        "coverage": o.coverage,
        "analyzers": o.analyzers,
        "semantic": o.semantic,
        "countedFindings": view.counted,
        "persisted": view.persisted,
        "changes": {
            "introduced": o.changes.introduced.len(),
            "fixed": o.changes.fixed.len(),
            "reopened": o.changes.reopened.len(),
        },
    }))
    .unwrap_or_default()
}

fn finding_lines(finding: &Finding) -> Vec<String> {
    let first = finding.locations.first();
    let file = first.map_or("", |l| l.file.as_str());
    let at = match (first.and_then(|l| l.lines), first.and_then(|l| l.columns)) {
        (None, _) => file.to_owned(),
        (Some(l), None) => format!("{file}:{}", l.start),
        (Some(l), Some(c)) => format!("{file}:{}:{}", l.start, c.start),
    };
    let tool = analyzer_of(finding).map_or_else(String::new, |(tool, version, license)| {
        format!(
            " · {tool}{}, {license}",
            version.map_or_else(String::new, |v| format!(" {v}"))
        )
    });
    let semantic = finding.evidence.iter().find_map(|e| match e {
        Evidence::SemanticReview {
            role,
            provider,
            model,
            verdict,
            reason,
            ..
        } => Some((role, provider, model, verdict, reason)),
        _ => None,
    });
    let who = |provider: &str, model: &Option<String>| {
        format!(
            "{provider}{}",
            model
                .as_deref()
                .map_or_else(String::new, |m| format!(" {m}"))
        )
    };
    let suspicion = match semantic {
        Some((SemanticRole::Detection, provider, model, ..)) => {
            format!(" · model suspicion, {}", who(provider, model))
        }
        _ => String::new(),
    };
    let mut lines = vec![format!(
        "{:<13} {at}  {}  ({}{tool}{suspicion})",
        severity_name(finding.severity),
        finding.title,
        rule_id_of(finding)
    )];
    if let Some(fix) = finding.recommendations.first() {
        lines.push(format!("{}→ {}", " ".repeat(14), fix.summary));
    }
    if let Some((SemanticRole::Verification, provider, model, Some(verdict), reason)) = semantic {
        let said = match verdict {
            SemanticVerdict::Confirmed => "thinks this is real",
            SemanticVerdict::Doubtful => "doubts this",
            SemanticVerdict::Unsure => "is unsure",
        };
        lines.push(format!(
            "{}model ({}) {said}: {reason}",
            " ".repeat(14),
            who(provider, model)
        ));
    }
    lines
}

fn text(view: &View<'_>) -> String {
    let mut sorted: Vec<&Finding> = view.findings.iter().collect();
    sorted.sort_by_key(|f| f.severity);
    let mut lines: Vec<String> = sorted.into_iter().flat_map(finding_lines).collect();
    if view.quiet {
        return lines.join("\n");
    }
    let o = view.output;
    let n = view.findings.len();
    lines.push(format!(
        "{n} {} (policy {}).",
        if n == 1 { "finding" } else { "findings" },
        o.policy.id
    ));
    lines.extend(view.notes.iter().cloned());
    if let Some(note) = &view.uncounted {
        lines.push(note.clone());
    }
    if view.ignored > 0 {
        lines.push(format!(
            "{} findings under paths the config ignores are left out.",
            view.ignored
        ));
    }
    if o.baselined > 0 {
        lines.push(format!(
            "{} findings accepted by the baseline are hidden (--include-baselined shows them).",
            o.baselined
        ));
    }
    if !o.suppressed_inline.is_empty() {
        lines.push(format!(
            "{} findings hidden by debuggatha-ignore comments.",
            o.suppressed_inline.len()
        ));
    }
    if !o.suppressed_by_memory.is_empty() {
        lines.push(format!(
            "{} suppressed by repository memory.",
            o.suppressed_by_memory.len()
        ));
    }
    if !o.coverage.complete {
        lines.push("Not everything was checked:".to_owned());
        for gap in &o.coverage.rule_gaps {
            lines.push(format!("  {} ({:?})", gap.rule_id, gap.reason));
        }
        for skipped in &o.coverage.skipped_files {
            lines.push(format!(
                "  {} files skipped ({:?})",
                skipped.files, skipped.reason
            ));
        }
    }
    lines.join("\n")
}

/// A line for each analyzer worth mentioning; silent about the ones the repository does not use.
pub(crate) fn describe_runs(runs: &[AnalyzerRun]) -> Vec<String> {
    let mut lines = Vec::new();
    for run in runs {
        match (run.status, run.reason.as_deref()) {
            (Status::Ran, _) => lines.push(format!(
                "{}{} ran: {} findings.",
                run.name,
                run.version
                    .as_deref()
                    .map_or_else(String::new, |v| format!(" {v}")),
                run.findings
            )),
            (Status::Failed, reason) => lines.push(format!(
                "{} failed ({}); its findings are not part of this review.",
                run.name,
                reason.unwrap_or("no reason given")
            )),
            (Status::Skipped, Some(reason)) if !unremarkable(reason) => {
                lines.push(format!("{} skipped: {reason}.", run.name));
            }
            (Status::Skipped, _) => {}
        }
    }
    lines
}

/// A tool the repository does not use is not news.
fn unremarkable(reason: &str) -> bool {
    ["nothing it analyzes", "no matching files", "turned off"]
        .iter()
        .any(|m| reason.contains(m))
}
