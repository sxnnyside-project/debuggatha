//! What a review looked at and what it could not check (`Coverage` in `docs/schemas`, PROTOCOL.md
//! section on `review/run`). An empty findings list means "clean" only for what is listed as
//! checked here.
//!
//! Built from a finished [`Run`], the policy and the registry, so nothing is parsed twice.

use std::collections::BTreeMap;
use std::path::Path;

use debuggatha_core::knowledge_system::{PolicyRuleOrigin, ReviewPolicy};
use serde::{Deserialize, Serialize};

use super::runner::{Registry, Run};
use crate::syntax::{Backend, Language, ParseStatus};

/// Rules that come from the repository's own criteria belong to no pack; the schema wants a pack
/// id and version, so they are reported under this one.
pub const CRITERIA_PACK_ID: &str = "criteria";
const CRITERIA_PACK_VERSION: &str = "repository";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CoverageStatus {
    Parsed,
    ParsedWithErrors,
    ParsedLimited,
    NotParsed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ParserKind {
    Oxc,
    TreeSitter,
    None,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileCoverage {
    pub path: String,
    pub status: CoverageStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LanguageCoverage {
    pub language: String,
    pub files: usize,
    pub files_with_errors: usize,
    pub status: CoverageStatus,
    pub parser: ParserKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_details: Option<Vec<FileCoverage>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GapReason {
    NoDetector,
    NeedsTypeInformation,
    /// The rule reads the repository's configuration and documentation files and the caller did
    /// not hand them over.
    NeedsRepositoryFiles,
    LanguageNotParsed,
    AnalyzerNotInstalled,
    AnalyzerNotEnabled,
    AnalyzerFailed,
    Timeout,
    DisabledByConfig,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleGap {
    pub pack_id: String,
    pub rule_id: String,
    pub reason: GapReason,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckedRules {
    pub pack_id: String,
    pub pack_version: String,
    pub rule_ids: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SkipReason {
    Ignored,
    Binary,
    Generated,
    SymlinkOutsideRoot,
    TooLarge,
    Unreadable,
    NotUtf8,
}

impl SkipReason {
    /// Scope decisions (`ignored`, `binary`, `generated`, `symlink-outside-root`) do not hide code
    /// the person asked to have reviewed; the others do.
    const fn hides_code(self) -> bool {
        matches!(self, Self::TooLarge | Self::Unreadable | Self::NotUtf8)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkippedFiles {
    pub reason: SkipReason,
    pub files: usize,
    /// At most five.
    pub examples: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Coverage {
    pub complete: bool,
    pub languages: Vec<LanguageCoverage>,
    pub checked_rules: Vec<CheckedRules>,
    pub rule_gaps: Vec<RuleGap>,
    pub skipped_files: Vec<SkippedFiles>,
}

/// What the parser says about one reviewed file, kept by the runner so coverage needs no reparse.
#[derive(Debug, Clone)]
pub struct ReviewedFile {
    pub file: String,
    /// `None`: the engine has no parser for the file.
    pub language: Option<Language>,
    pub status: Option<ParseStatus>,
}

fn pack_of(origin: &PolicyRuleOrigin) -> (String, String) {
    match origin {
        PolicyRuleOrigin::Pack {
            pack_id,
            pack_version,
        } => (pack_id.clone(), pack_version.clone()),
        PolicyRuleOrigin::Criteria => (
            CRITERIA_PACK_ID.to_owned(),
            CRITERIA_PACK_VERSION.to_owned(),
        ),
    }
}

const fn parser_of(language: Language) -> ParserKind {
    // Vue, Svelte and Astro: the HTML grammar finds the scripts, oxc reads them; the code that is
    // analyzed is oxc's.
    match language.backend() {
        Backend::Oxc | Backend::Sfc => ParserKind::Oxc,
        Backend::TreeSitter => ParserKind::TreeSitter,
    }
}

const fn limit_of(language: Language) -> &'static str {
    match language {
        Language::Cpp => {
            "the grammar does not run the preprocessor, so macro-heavy code reads as errors"
        }
        _ => "only the scripts are parsed; templates and styles are not",
    }
}

fn file_status(file: &ReviewedFile) -> (CoverageStatus, Option<String>) {
    match (file.language, file.status) {
        (Some(language), Some(ParseStatus::Parsed)) if language.limited() => (
            CoverageStatus::ParsedLimited,
            Some(limit_of(language).to_owned()),
        ),
        (Some(_), Some(ParseStatus::Parsed)) => (CoverageStatus::Parsed, None),
        (Some(_), Some(ParseStatus::ParsedWithErrors)) => (
            CoverageStatus::ParsedWithErrors,
            Some(
                "syntax errors: the intact parts were analyzed, the broken regions were not".into(),
            ),
        ),
        (Some(_), Some(ParseStatus::Failed)) => (
            CoverageStatus::NotParsed,
            Some("a fatal parse error: nothing is concluded about this file".into()),
        ),
        _ => (
            CoverageStatus::NotParsed,
            Some("no parser for this language: only text-level rules ran".into()),
        ),
    }
}

/// The language a file is reported under: the parser's, or its extension when there is none.
fn language_key(file: &ReviewedFile) -> String {
    if let Some(language) = file.language {
        return language.id().to_owned();
    }
    Path::new(&file.file)
        .extension()
        .and_then(|e| e.to_str())
        .map_or_else(|| "unknown".to_owned(), str::to_lowercase)
}

/// A file with the status and reason it was given.
type Member<'a> = (&'a ReviewedFile, CoverageStatus, Option<String>);

fn languages_of(files: &[ReviewedFile], detail: bool) -> Vec<LanguageCoverage> {
    let mut groups: BTreeMap<String, Vec<Member<'_>>> = BTreeMap::new();
    for file in files {
        let (status, reason) = file_status(file);
        groups
            .entry(language_key(file))
            .or_default()
            .push((file, status, reason));
    }
    groups
        .into_iter()
        .map(|(language, members)| {
            let count = |s: CoverageStatus| members.iter().filter(|m| m.1 == s).count();
            let with_errors =
                count(CoverageStatus::ParsedWithErrors) + count(CoverageStatus::NotParsed);
            let first = members[0].0;
            let parsed = first.language.is_some();
            let (status, reason) = if !parsed {
                (CoverageStatus::NotParsed, members[0].2.clone())
            } else if with_errors > 0 {
                let fatal = count(CoverageStatus::NotParsed);
                let mut parts = vec![format!("{with_errors} file(s) with syntax errors")];
                if fatal > 0 {
                    parts.push(format!("{fatal} not readable at all"));
                }
                let mut why = parts.join(", ");
                if let Some(l) = first.language.filter(|l| l.limited()) {
                    why = format!("{why}; also, {}", limit_of(l));
                }
                (CoverageStatus::ParsedWithErrors, Some(why))
            } else if let Some(l) = first.language.filter(|l| l.limited()) {
                (CoverageStatus::ParsedLimited, Some(limit_of(l).to_owned()))
            } else {
                (CoverageStatus::Parsed, None)
            };
            LanguageCoverage {
                language,
                files: members.len(),
                files_with_errors: if parsed { with_errors } else { 0 },
                status,
                parser: first.language.map_or(ParserKind::None, parser_of),
                reason,
                file_details: detail.then(|| {
                    members
                        .iter()
                        .map(|(f, s, r)| FileCoverage {
                            path: f.file.clone(),
                            status: *s,
                            reason: r.clone(),
                        })
                        .collect()
                }),
            }
        })
        .collect()
}

impl Run {
    /// The coverage of this run. `skipped` is what the caller left out of the scope before the
    /// engine saw it (too large, not UTF-8...): the engine cannot know, so it is stated, not
    /// guessed. `detail` adds one entry per file.
    #[must_use]
    pub fn coverage(
        &self,
        policy: &ReviewPolicy,
        registry: &Registry,
        skipped: Vec<SkippedFiles>,
        detail: bool,
    ) -> Coverage {
        let mut checked: BTreeMap<(String, String), Vec<String>> = BTreeMap::new();
        let mut rule_gaps = Vec::new();
        let mut reviewed: Vec<Language> = self.files.iter().filter_map(|f| f.language).collect();
        reviewed.sort_by_key(|l| l.id());
        reviewed.dedup();
        for rule in &policy.rules {
            let (pack_id, pack_version) = pack_of(&rule.origin);
            if registry.has(&rule.id)
                && !self.repo_files_given
                && super::rules::env::NEEDS_REPO_FILES.contains(&rule.id.as_str())
            {
                // It could not run: not checked, and the review says why.
                rule_gaps.push(RuleGap {
                    pack_id,
                    rule_id: rule.id.clone(),
                    reason: GapReason::NeedsRepositoryFiles,
                    detail: Some(
                        "the review was given no configuration or documentation files".to_owned(),
                    ),
                });
            } else if registry.has(&rule.id) {
                let ids = checked.entry((pack_id, pack_version)).or_default();
                if !ids.contains(&rule.id) {
                    ids.push(rule.id.clone());
                }
                let unbound = super::rules::concepts::unbound_languages(&rule.id, &reviewed);
                if !unbound.is_empty() {
                    rule_gaps.push(RuleGap {
                        pack_id: pack_of(&rule.origin).0,
                        rule_id: rule.id.clone(),
                        reason: GapReason::NoDetector,
                        detail: Some(format!("not checked in {}", unbound.join(", "))),
                    });
                }
                // Checked, but not all of it: the rest needs the types the engine does not have.
                if let Some((reason, left_out)) = registry.partial_gap(&rule.id) {
                    rule_gaps.push(RuleGap {
                        pack_id: pack_of(&rule.origin).0,
                        rule_id: rule.id.clone(),
                        reason,
                        detail: Some(left_out.to_owned()),
                    });
                }
            } else {
                rule_gaps.push(RuleGap {
                    pack_id,
                    rule_id: rule.id.clone(),
                    reason: GapReason::NoDetector,
                    detail: None,
                });
            }
        }
        let skipped_files: Vec<SkippedFiles> = skipped
            .into_iter()
            .filter(|s| s.files > 0)
            .map(|mut s| {
                s.examples.truncate(5);
                s
            })
            .collect();
        let languages = languages_of(&self.files, detail);
        let complete = languages.iter().all(|l| l.status == CoverageStatus::Parsed)
            && rule_gaps
                .iter()
                .all(|g| g.reason == GapReason::DisabledByConfig)
            && !skipped_files.iter().any(|s| s.reason.hides_code());
        Coverage {
            complete,
            languages,
            checked_rules: checked
                .into_iter()
                .map(|((pack_id, pack_version), rule_ids)| CheckedRules {
                    pack_id,
                    pack_version,
                    rule_ids,
                })
                .collect(),
            rule_gaps,
            skipped_files,
        }
    }
}
