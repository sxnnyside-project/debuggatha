//! Asks a model about what a review found and what changed, and returns the findings with its
//! opinions attached. It never removes a finding, and it fails soft: a model that is down or
//! confused leaves the review exactly as the analyzers made it.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::time::Instant;

use debuggatha_core::env::Env;
use debuggatha_core::review_engine::{
    Confidence, Evidence, Finding, FindingError, SemanticVerdict, Severity, rule_id_from_evidence,
};

use super::judge::{detection_prompt, detections_from, judge, with_verdict};
use super::types::{SemanticOptions, SemanticProvider, SemanticRun};
use super::view::{CodeLine, CodeView, is_secret_file, window_around};
use crate::diff::FileDiff;
use crate::scan_scope::is_ignored_path;
use crate::syntax::Language;

/// Calls in a row that may fail before the model is given up on for this review.
const MAX_CONSECUTIVE_FAILURES: u32 = 3;

/// What the review was asked about: the only code a model may read.
pub enum Scope<'a> {
    /// A whole-repository review has no changed code, so only candidates are verified.
    Workspace,
    Diff(&'a [FileDiff]),
    Files(&'a [String]),
}

pub struct Refine<'a> {
    pub root: &'a Path,
    pub scope: Scope<'a>,
    pub findings: Vec<Finding>,
    pub policy_id: &'a str,
    pub options: &'a SemanticOptions,
    pub provider: &'a dyn SemanticProvider,
    pub env: &'a dyn Env,
}

fn is_secret_rule(rule: Option<&str>) -> bool {
    rule.is_some_and(|r| r == "no-hardcoded-secrets" || r.starts_with("gitleaks:"))
}

fn severity_rank(severity: Severity) -> u8 {
    match severity {
        Severity::Critical => 0,
        Severity::High => 1,
        Severity::Medium => 2,
        Severity::Low => 3,
        Severity::Informational => 4,
    }
}

/// A file a model is shown: one the engine reads as code.
fn is_code(file: &str) -> bool {
    Language::from_path(Path::new(file)).is_some_and(|l| l != Language::Html)
}

fn detection_views(root: &Path, scope: &Scope<'_>, max_lines: usize) -> Vec<CodeView> {
    match scope {
        Scope::Workspace => Vec::new(),
        Scope::Diff(files) => files
            .iter()
            .filter(|f| !f.binary && !f.added.is_empty())
            .map(|f| CodeView {
                file: f.path.clone(),
                lines: f
                    .added
                    .iter()
                    .take(max_lines)
                    .map(|a| CodeLine {
                        n: a.line,
                        text: a.text.clone(),
                    })
                    .collect(),
            })
            .collect(),
        Scope::Files(paths) => paths
            .iter()
            .filter_map(|path| {
                // A file that is gone has nothing to read.
                let text = crate::diff_review::read_text(root, path).ok()?;
                Some(CodeView {
                    file: path.clone(),
                    lines: text
                        .split('\n')
                        .take(max_lines)
                        .zip(1_u32..)
                        .map(|(line, n)| CodeLine {
                            n,
                            text: line.to_owned(),
                        })
                        .collect(),
                })
            })
            .collect(),
    }
}

struct Pass<'a> {
    run: SemanticRun,
    consecutive: u32,
    provider: &'a dyn SemanticProvider,
}

/// Lines of each file that may hold a secret, which a model is never shown.
struct Hidden<'a> {
    by_file: BTreeMap<&'a str, BTreeSet<u32>>,
    none: BTreeSet<u32>,
}

impl<'a> Hidden<'a> {
    fn of_findings(findings: &'a [Finding]) -> Self {
        let mut by_file: BTreeMap<&str, BTreeSet<u32>> = BTreeMap::new();
        for finding in findings {
            let rule = rule_id_from_evidence(&finding.evidence);
            if let (true, Some(location)) =
                (is_secret_rule(rule.as_deref()), finding.locations.first())
                && let Some(lines) = location.lines
            {
                by_file
                    .entry(location.file.as_str())
                    .or_default()
                    .extend(lines.start..=lines.end);
            }
        }
        Self {
            by_file,
            none: BTreeSet::new(),
        }
    }

    fn of(&self, file: &str) -> &BTreeSet<u32> {
        self.by_file.get(file).unwrap_or(&self.none)
    }
}

impl Pass<'_> {
    fn alive(&self) -> bool {
        self.consecutive < MAX_CONSECUTIVE_FAILURES
    }

    fn failed(&mut self, why: &str) {
        self.run.failed += 1;
        self.consecutive += 1;
        if self.run.notes.len() < 3 {
            self.run.notes.push(why.to_owned());
        }
    }

    fn count(&mut self, verdict: SemanticVerdict) {
        self.run.verified += 1;
        match verdict {
            SemanticVerdict::Confirmed => self.run.confirmed += 1,
            SemanticVerdict::Doubtful => self.run.doubtful += 1,
            SemanticVerdict::Unsure => self.run.unsure += 1,
        }
    }

    /// Attaches a verdict to the findings the detectors are unsure of, most severe first.
    fn verify(
        &mut self,
        root: &Path,
        findings: &mut [Finding],
        options: &SemanticOptions,
        hidden: &Hidden<'_>,
    ) {
        let mut candidates: Vec<&Finding> = findings.iter().filter(|f| is_candidate(f)).collect();
        candidates.sort_by_key(|f| severity_rank(f.severity));
        candidates.truncate(options.max_candidates);

        let mut verdicts: BTreeMap<String, Finding> = BTreeMap::new();
        for finding in candidates {
            if !self.alive() {
                break;
            }
            let Some(location) = finding.locations.first() else {
                continue;
            };
            let Some(line) = location.lines.map(|l| l.start) else {
                continue;
            };
            let Some(view) = window_around(root, &location.file, line, 8) else {
                continue;
            };
            match judge(self.provider, finding, &view, hidden.of(&location.file)) {
                Ok(Some(verification)) => {
                    self.consecutive = 0;
                    self.count(verification.verdict);
                    verdicts.insert(
                        finding.id.clone(),
                        with_verdict(finding, &verification, self.provider),
                    );
                }
                Ok(None) => {
                    self.run.failed += 1;
                    self.consecutive += 1;
                }
                Err(why) => self.failed(&why),
            }
        }
        for finding in findings {
            if let Some(with) = verdicts.remove(&finding.id) {
                *finding = with;
            }
        }
    }

    /// Reads the changed code for what no analyzer sees; what a detector already said about a line
    /// is not said again.
    fn detect(
        &mut self,
        views: &[CodeView],
        existing: &[Finding],
        hidden: &Hidden<'_>,
        context: (&str, &dyn Env),
    ) -> Result<Vec<Finding>, FindingError> {
        let at = |f: &Finding| {
            f.locations
                .first()
                .map(|l| (l.file.clone(), l.lines.map(|r| r.start)))
        };
        let mut added = Vec::new();
        for view in views {
            if !self.alive() {
                break;
            }
            let prompt = detection_prompt(view, hidden.of(&view.file));
            match self.provider.complete(&prompt) {
                Err(why) => self.failed(&why),
                Ok(reply) => {
                    self.consecutive = 0;
                    let result =
                        detections_from(&reply, view, self.provider, context.0, context.1)?;
                    self.run.rejected += result.rejected;
                    added.extend(
                        result
                            .accepted
                            .into_iter()
                            .filter(|f| !existing.iter().any(|e| at(e) == at(f))),
                    );
                }
            }
        }
        self.run.detected = u32::try_from(added.len()).unwrap_or(u32::MAX);
        Ok(added)
    }
}

/// A finding a model may be asked about: one the detectors are unsure of, that is not about a
/// secret, not in a secret file, and that no analyzer or model already spoke to.
fn is_candidate(finding: &Finding) -> bool {
    let rule = rule_id_from_evidence(&finding.evidence);
    let file = finding.locations.first().map_or("", |l| l.file.as_str());
    finding.confidence != Confidence::High
        && !is_secret_rule(rule.as_deref())
        && !is_secret_file(file)
        && !finding.evidence.iter().any(|e| {
            matches!(
                e,
                Evidence::ExternalAnalyzer { .. } | Evidence::SemanticReview { .. }
            )
        })
        && finding.locations.first().is_some_and(|l| l.lines.is_some())
}

/// # Errors
/// A finding the contract refuses; a detection always has a file, a line and evidence.
pub fn refine_findings(input: Refine<'_>) -> Result<(Vec<Finding>, SemanticRun), FindingError> {
    let Refine {
        root,
        scope,
        findings: original,
        policy_id,
        options,
        provider,
        env,
    } = input;
    let started = Instant::now();
    let mut pass = Pass {
        run: SemanticRun {
            provider: provider.name().to_owned(),
            model: provider.model(),
            ..SemanticRun::default()
        },
        consecutive: 0,
        provider,
    };
    let hidden = Hidden::of_findings(&original);
    let mut findings = original.clone();

    if options.verify {
        pass.verify(root, &mut findings, options, &hidden);
    }
    if options.detect && matches!(scope, Scope::Workspace) {
        pass.run.notes.push(
            "Detection reads changed code; a whole-repository review has none, so only candidates were verified."
                .to_owned(),
        );
    } else if options.detect {
        let views: Vec<CodeView> = detection_views(root, &scope, options.max_lines)
            .into_iter()
            .filter(|v| {
                is_code(&v.file)
                    && !is_secret_file(&v.file)
                    && !is_ignored_path(Some(root), &v.file)
            })
            .take(options.max_detect_files)
            .collect();
        let added = pass.detect(&views, &findings, &hidden, (policy_id, env))?;
        findings.extend(added);
    }
    if !pass.alive() {
        pass.run.notes.push(
            "The model failed repeatedly, so the rest of the semantic pass was skipped.".to_owned(),
        );
    }
    // A host reveals its model only in its first answer.
    pass.run.model = pass.provider.model();
    pass.run.duration_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    Ok((findings, pass.run))
}
