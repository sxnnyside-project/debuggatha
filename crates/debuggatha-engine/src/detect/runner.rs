//! The one place that turns a resolved `ReviewPolicy` and a set of source units into findings.
//! Review of a diff, of files and of a workspace differ only in what units they hand it (port of
//! `runReviewSkillsEngine`).
//!
//! Deterministic: the same policy and units give the same findings in the same order (rules in
//! policy order, files in input order, lines top to bottom). No model, no randomness.

use std::collections::{BTreeMap, HashMap};

use debuggatha_core::env::Env;
use debuggatha_core::js::js_trim;
use debuggatha_core::knowledge_system::{PolicyRuleOrigin, ReviewPolicy};
use debuggatha_core::review_engine::{
    CreateFindingInput, Evidence, Finding, FindingError, FindingLocation, LineRange, PackReference,
    PolicyReference, Recommendation, RecommendationAction, RecommendationEdit,
    RecommendationExample, Severity, create_finding,
};

use super::coverage::{GapReason, ReviewedFile};
use super::guidance::guidance_for;
use super::hit::Hit;
use super::input::{DetectorInput, RepoFile, SourceUnit, parse_unit};
use super::rules::env_docs::EnvDocs;
use super::rules::modules::ModuleIndex;
use crate::control::{Control, Stage};
use crate::syntax::{
    InlineSuppression, ParseStatus, comment_lines, find_directive, parse_directives,
};

/// A detector for one rule.
pub type Detector = fn(&DetectorInput<'_>) -> Vec<Hit>;
/// A detector for several rules that share one parse: it returns each hit tagged with its rule id.
pub type GroupDetector = fn(&DetectorInput<'_>) -> Vec<(&'static str, Hit)>;

#[derive(Clone)]
enum Entry {
    /// A detector and whether it needs a parsed file.
    Single(Detector, bool),
    /// Indices into `Registry::groups`; group detectors always need a parsed file. A rule that
    /// spans languages (`no-eval` in JavaScript and PHP) has one group per backend.
    Groups(Vec<usize>),
}

/// Rule id to detector. Nothing is registered by default: a rule with no entry is a gap the review
/// reports, never a silent pass.
#[derive(Default, Clone)]
pub struct Registry {
    entries: BTreeMap<String, Entry>,
    groups: Vec<GroupDetector>,
    /// Rules whose detector checks only part of what the rule says, and what is left out.
    partial: BTreeMap<String, (GapReason, &'static str)>,
}

impl Registry {
    /// A registry with no detectors.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// A detector that reads the syntax tree: it is not run on a file the engine has no parser for
    /// or could not parse (the file is reported as not analyzed instead).
    pub fn register(&mut self, rule_id: &str, detector: Detector) {
        self.entries
            .insert(rule_id.to_owned(), Entry::Single(detector, true));
    }

    /// A detector whose rule is about text, not syntax (a credential has a format in any file).
    pub fn register_text(&mut self, rule_id: &str, detector: Detector) {
        self.entries
            .insert(rule_id.to_owned(), Entry::Single(detector, false));
    }

    /// One detector for several rules, so the file is parsed once. Needs a parsed file.
    pub fn register_group(&mut self, rule_ids: &[&str], detector: GroupDetector) {
        let index = self.groups.len();
        self.groups.push(detector);
        for id in rule_ids {
            match self.entries.get_mut(*id) {
                Some(Entry::Groups(groups)) => groups.push(index),
                _ => {
                    self.entries
                        .insert((*id).to_owned(), Entry::Groups(vec![index]));
                }
            }
        }
    }

    /// The detector of `rule_id` checks only part of the rule (a typed rule read without types):
    /// coverage reports the rest as a gap that needs type information, so a review never calls
    /// the rule fully checked.
    pub fn mark_partial(&mut self, rule_id: &str, left_out: &'static str) {
        self.mark_partial_as(rule_id, GapReason::NeedsTypeInformation, left_out);
    }

    /// The same, for a rule that leaves something out for another reason (a language whose
    /// source the rule has no reader for).
    pub fn mark_partial_as(&mut self, rule_id: &str, reason: GapReason, left_out: &'static str) {
        self.partial.insert(rule_id.to_owned(), (reason, left_out));
    }

    /// What the detector of a partially checked rule leaves out.
    #[must_use]
    pub fn partial(&self, rule_id: &str) -> Option<&'static str> {
        self.partial.get(rule_id).map(|(_, left_out)| *left_out)
    }

    /// Why the rule is only partly checked, and what is left out.
    #[must_use]
    pub fn partial_gap(&self, rule_id: &str) -> Option<(GapReason, &'static str)> {
        self.partial.get(rule_id).copied()
    }

    /// Does the rule have a detector?
    #[must_use]
    pub fn has(&self, rule_id: &str) -> bool {
        self.entries.contains_key(rule_id)
    }

    /// Rule ids that have a detector.
    pub fn rule_ids(&self) -> impl Iterator<Item = &str> {
        self.entries.keys().map(String::as_str)
    }
}

/// Why syntax detectors did not run on a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum NotAnalyzed {
    /// The engine has no parser for the language.
    NoParser,
    /// The parser found a fatal error: nothing is concluded about the file.
    ParseFailed,
}

/// What a run produced: the findings, what inline directives hid (so suppression stays visible),
/// and the files syntax detectors could not read (so a review never calls them clean).
#[derive(Debug)]
pub struct Run {
    /// What the detectors found, after inline suppression.
    pub findings: Vec<Finding>,
    /// What inline directives hid.
    pub suppressed: Vec<InlineSuppression>,
    /// Files a syntax rule could not read, by path.
    pub not_analyzed: BTreeMap<String, NotAnalyzed>,
    /// What the parser made of each unit, in input order (see [`Run::coverage`]).
    pub files: Vec<ReviewedFile>,
    /// Did the caller hand over the repository's configuration and documentation files? A rule that
    /// reads them (`env::NEEDS_REPO_FILES`) cannot conclude without; coverage says so.
    pub repo_files_given: bool,
    /// The run was stopped before every rule ran; its findings are partial.
    pub cancelled: bool,
}

/// The first sentence of a rule's statement, which is the finding's title.
fn title_of(statement: &str) -> String {
    let first = statement.split(['.', '\n']).next().map_or("", js_trim);
    if first.is_empty() {
        statement.to_owned()
    } else {
        first.to_owned()
    }
}

/// A unit with its parse and the directives its comments hold.
struct Prepared<'u> {
    unit: &'u SourceUnit,
    syntax: Option<crate::syntax::FileSyntax>,
    directives: Vec<crate::syntax::Directive>,
}

fn prepare(units: &[SourceUnit]) -> Vec<Prepared<'_>> {
    units
        .iter()
        .map(|unit| {
            let syntax = parse_unit(unit);
            let directives = syntax
                .as_ref()
                .and_then(|s| s.facts.as_ref())
                .map(|facts| parse_directives(&comment_lines(&unit.content, facts)))
                .unwrap_or_default();
            Prepared {
                unit,
                syntax,
                directives,
            }
        })
        .collect()
}

/// # Errors
/// A detector that returns a hit without a location cannot happen; the only failure is the
/// finding contract itself (no locations, no evidence), which this function always satisfies.
pub fn run_review(
    policy: &ReviewPolicy,
    units: &[SourceUnit],
    registry: &Registry,
    env: &dyn Env,
) -> Result<Run, FindingError> {
    run_review_with_files(policy, units, None, registry, env)
}

/// [`run_review`] with the files of the repository that are read for what they say rather than
/// reviewed as code (dotenv examples, README, Dockerfile, compose, CI, package manager manifests).
/// `None` means the caller did not have them: the rules that need them report nothing and
/// coverage lists them as a gap; `Some(&[])` means there are none, and every environment variable
/// the code reads is undocumented.
///
/// # Errors
/// As [`run_review`].
pub fn run_review_with_files(
    policy: &ReviewPolicy,
    units: &[SourceUnit],
    repo_files: Option<&[RepoFile]>,
    registry: &Registry,
    env: &dyn Env,
) -> Result<Run, FindingError> {
    run_review_controlled(policy, units, repo_files, registry, env, &Control::none())
}

/// [`run_review_with_files`] that can be stopped between rules and reports how many rules it has
/// run. A stopped run returns what it has found so far, with `cancelled` set; nothing in it is a
/// complete answer.
///
/// # Errors
/// As [`run_review`].
pub fn run_review_controlled(
    policy: &ReviewPolicy,
    units: &[SourceUnit],
    repo_files: Option<&[RepoFile]>,
    registry: &Registry,
    env: &dyn Env,
    control: &Control,
) -> Result<Run, FindingError> {
    let prepared = prepare(units);

    let mut run = Run {
        findings: Vec::new(),
        suppressed: Vec::new(),
        not_analyzed: BTreeMap::new(),
        files: prepared
            .iter()
            .map(|p| ReviewedFile {
                file: p.unit.file.clone(),
                language: p.syntax.as_ref().map(|s| s.language),
                status: p.syntax.as_ref().map(|s| s.status),
            })
            .collect(),
        repo_files_given: repo_files.is_some(),
        cancelled: false,
    };
    let env_docs = if policy
        .rules
        .iter()
        .any(|r| super::rules::env::NEEDS_REPO_FILES.contains(&r.id.as_str()))
    {
        repo_files.map(EnvDocs::build)
    } else {
        None
    };
    // A group detector runs once per file, whatever number of its rules the policy holds.
    let index = ModuleIndex::for_policy(policy, units);
    let mut memo: HashMap<(usize, usize), Vec<(&'static str, Hit)>> = HashMap::new();
    for (done, rule) in policy.rules.iter().enumerate() {
        if control.cancelled() {
            run.cancelled = true;
            return Ok(run);
        }
        control.report(Stage::Detect, done, policy.rules.len(), "");
        let Some(entry) = registry.entries.get(&rule.id) else {
            continue;
        };
        let needs_syntax = match entry {
            Entry::Single(_, needs) => *needs,
            Entry::Groups(_) => true,
        };
        let guidance = guidance_for(&rule.id);

        for (unit_index, p) in prepared.iter().enumerate() {
            if needs_syntax {
                let why = match p.syntax.as_ref().map(|s| s.status) {
                    None => Some(NotAnalyzed::NoParser),
                    Some(ParseStatus::Failed) => Some(NotAnalyzed::ParseFailed),
                    Some(_) => None,
                };
                if let Some(why) = why {
                    run.not_analyzed.insert(p.unit.file.clone(), why);
                    continue;
                }
            }
            let input = DetectorInput::new(&p.unit.file, &p.unit.content, p.syntax.as_ref())
                .with_modules(index.as_ref())
                .with_env_docs(env_docs.as_ref());
            let hits: Vec<Hit> = match entry {
                Entry::Single(detector, _) => detector(&input),
                Entry::Groups(groups) => {
                    let mut hits = Vec::new();
                    for &g in groups {
                        let all = memo
                            .entry((unit_index, g))
                            .or_insert_with(|| (registry.groups[g])(&input));
                        hits.extend(
                            all.iter()
                                .filter(|(id, _)| *id == rule.id)
                                .map(|(_, hit)| hit.clone()),
                        );
                    }
                    hits
                }
            };
            for hit in hits {
                if p.unit
                    .changed_lines
                    .as_ref()
                    .is_some_and(|set| !set.contains(&hit.line))
                {
                    continue;
                }
                if let Some(directive) = find_directive(&p.directives, hit.line, &rule.id) {
                    run.suppressed.push(InlineSuppression {
                        file: p.unit.file.clone(),
                        line: Some(hit.line),
                        rule_id: rule.id.clone(),
                        reason: directive.reason.clone(),
                    });
                    continue;
                }
                run.findings
                    .push(finding_for(policy, rule, p.unit, &hit, guidance, env)?);
            }
        }
    }
    Ok(run)
}

fn finding_for(
    policy: &ReviewPolicy,
    rule: &debuggatha_core::knowledge_system::PolicyRule,
    unit: &SourceUnit,
    hit: &Hit,
    guidance: Option<&super::guidance::Guidance>,
    env: &dyn Env,
) -> Result<Finding, FindingError> {
    let lines = LineRange {
        start: hit.line,
        end: hit.line,
    };
    let mut evidence = vec![Evidence::Code {
        file: unit.file.clone(),
        lines: Some(lines),
        excerpt: Some(hit.excerpt.clone()),
    }];
    let originating_pack = match &rule.origin {
        PolicyRuleOrigin::Pack {
            pack_id,
            pack_version,
        } => {
            evidence.push(Evidence::ReviewPackRule {
                pack_id: pack_id.clone(),
                rule_id: rule.id.clone(),
            });
            Some(PackReference {
                id: pack_id.clone(),
                version: pack_version.clone(),
            })
        }
        PolicyRuleOrigin::Criteria => {
            evidence.push(Evidence::Criteria {
                rule_id: rule.id.clone(),
                source: unit.file.clone(),
            });
            None
        }
    };

    let recommendation = Recommendation {
        id: format!("{}-{}-{}", rule.id, unit.file, hit.line),
        action: guidance.map_or(RecommendationAction::Refactor, |g| g.action),
        summary: guidance.map_or_else(
            || format!("Address \"{}\": {}", rule.id, rule.statement),
            |g| g.fix.clone(),
        ),
        rationale: Some(guidance.map_or_else(|| rule.statement.clone(), |g| g.why.clone())),
        target_file: Some(unit.file.clone()),
        target_lines: Some(lines),
        example: guidance.map(|g| RecommendationExample {
            before: g.before.clone(),
            after: g.after.clone(),
        }),
        edit: hit.edit.as_ref().map(|e| RecommendationEdit {
            line: hit.line,
            start_column: e.start_column,
            end_column: e.end_column,
            replacement: e.replacement.clone(),
            safety: e.safety,
        }),
    };

    create_finding(
        CreateFindingInput {
            id: None,
            title: title_of(&rule.statement),
            explanation: guidance.map_or_else(
                || format!("Rule \"{}\": {}", rule.id, rule.statement),
                |g| g.why.clone(),
            ),
            severity: hit
                .severity
                .or(rule.default_severity)
                .unwrap_or(Severity::Medium),
            confidence: hit.confidence,
            category: rule
                .category
                .clone()
                .unwrap_or_else(|| "maintainability".to_owned()),
            locations: vec![FindingLocation {
                file: unit.file.clone(),
                lines: Some(lines),
                columns: Some(hit.columns),
            }],
            evidence,
            applied_policy: Some(PolicyReference {
                id: policy.id.clone(),
            }),
            originating_pack,
            recommendations: vec![recommendation],
        },
        env,
    )
}
