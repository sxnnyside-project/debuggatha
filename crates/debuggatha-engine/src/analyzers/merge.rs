//! Adds what the analyzers found to what the built-in detectors found, without saying the same
//! thing twice. A secret scanner answers for secrets entirely, so the built-in secret heuristics
//! step aside when one ran.

use std::collections::{BTreeSet, HashMap};
use std::path::Path;

use debuggatha_core::env::Env;
use debuggatha_core::review_engine::{
    CreateFindingInput, Evidence, Finding, FindingError, FindingLocation, LineRange,
    PolicyReference, Recommendation, RecommendationAction, create_finding, rule_id_from_evidence,
};

use super::runner::AnalyzerFindings;
use super::types::AnalyzerFinding;
use crate::detect::excerpt_of;

/// Reads lines of the reviewed files once, so an excerpt costs one read per file.
struct SourceLines<'a> {
    root: &'a Path,
    cache: HashMap<String, Option<Vec<String>>>,
}

impl SourceLines<'_> {
    fn line(&mut self, file: &str, line: Option<u32>) -> Option<String> {
        let line = usize::try_from(line?).ok()?.checked_sub(1)?;
        let lines = self.cache.entry(file.to_owned()).or_insert_with(|| {
            std::fs::read_to_string(self.root.join(file))
                .ok()
                .map(|t| t.split('\n').map(str::to_owned).collect())
        });
        lines.as_ref()?.get(line).map(|l| excerpt_of(l))
    }
}

/// One analyzer finding as a Debuggatha finding, carrying the tool, its license and its rule as
/// evidence.
fn to_finding(
    found: &AnalyzerFinding,
    results: &AnalyzerFindings,
    lines: &mut SourceLines<'_>,
    policy_id: &str,
    env: &dyn Env,
) -> Result<Finding, FindingError> {
    let adapter = results.adapter;
    let excerpt = if found.sensitive {
        None
    } else {
        lines
            .line(&found.file, found.line)
            .or_else(|| found.line.is_none().then(|| excerpt_of(&found.message)))
    };
    let range = found.line.map(|start| LineRange {
        start,
        end: found.end_line.unwrap_or(start),
    });
    // A column range only means something on one line.
    let one_line = found.end_line.or(found.line) == found.line;
    let columns = found.column.map(|start| LineRange {
        start,
        end: if one_line {
            found.end_column.unwrap_or(start)
        } else {
            start
        },
    });
    let url = found.url.clone();
    create_finding(
        CreateFindingInput {
            id: None,
            title: found.message.clone(),
            explanation: format!(
                "{} rule {}{}. Debuggatha ran {} and reports what it found; the repository's own configuration decides which rules apply.",
                adapter.name,
                found.rule_id,
                url.as_ref().map_or(String::new(), |u| format!(" ({u})")),
                adapter.name
            ),
            severity: found.severity,
            confidence: debuggatha_core::review_engine::Confidence::High,
            category: found.category.clone(),
            locations: vec![FindingLocation {
                file: found.file.clone(),
                lines: range,
                columns,
            }],
            evidence: vec![
                Evidence::Code {
                    file: found.file.clone(),
                    lines: range,
                    excerpt,
                },
                Evidence::ExternalAnalyzer {
                    tool: adapter.id.to_owned(),
                    rule_id: found.rule_id.clone(),
                    version: results.version.clone(),
                    license: adapter.license.to_owned(),
                    url: url.clone(),
                },
            ],
            applied_policy: Some(PolicyReference {
                id: policy_id.to_owned(),
            }),
            originating_pack: None,
            recommendations: vec![Recommendation {
                id: format!(
                    "{}-{}-{}-{}",
                    adapter.id,
                    found.rule_id,
                    found.file,
                    found.line.unwrap_or(0)
                ),
                action: RecommendationAction::Investigate,
                summary: format!(
                    "Resolve {} {}: {}",
                    adapter.name, found.rule_id, found.message
                ),
                rationale: url.map(|u| format!("See {u}")),
                target_file: Some(found.file.clone()),
                target_lines: range,
                example: None,
                edit: None,
            }],
        },
        env,
    )
}

/// # Errors
/// A finding the contract refuses; an analyzer finding always has a file and evidence.
pub fn merge_analyzer_findings(
    internal: Vec<Finding>,
    results: &[AnalyzerFindings],
    root: &Path,
    policy_id: &str,
    env: &dyn Env,
) -> Result<Vec<Finding>, FindingError> {
    let superseded: BTreeSet<&str> = results
        .iter()
        .flat_map(|r| r.adapter.supersedes.iter().copied())
        .collect();
    // Where a tool reports the same rule on the same line, its finding replaces ours.
    let covered: BTreeSet<(&str, u32, &str)> = results
        .iter()
        .flat_map(|r| {
            r.findings.iter().filter_map(move |f| {
                let internal = r
                    .adapter
                    .equivalent_to
                    .iter()
                    .find(|(tool, _)| *tool == f.rule_id)?
                    .1;
                Some((f.file.as_str(), f.line?, internal))
            })
        })
        .collect();
    let mut merged: Vec<Finding> = internal
        .into_iter()
        .filter(|f| {
            let Some(id) = rule_id_from_evidence(&f.evidence) else {
                return true;
            };
            let at = f
                .locations
                .first()
                .and_then(|l| Some((l.file.as_str(), l.lines?.start)));
            !superseded.contains(id.as_str())
                && !at.is_some_and(|(file, line)| covered.contains(&(file, line, id.as_str())))
        })
        .collect();
    let mut lines = SourceLines {
        root,
        cache: HashMap::new(),
    };
    for result in results {
        for found in &result.findings {
            merged.push(to_finding(found, result, &mut lines, policy_id, env)?);
        }
    }
    Ok(merged)
}

/// The ids of the analyzers a repository could have, so ledger entries of one that did not run
/// are recognizable.
#[must_use]
pub fn analyzer_ids() -> BTreeSet<&'static str> {
    super::adapters::ADAPTERS.iter().map(|a| a.id).collect()
}
