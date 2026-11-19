//! Diff review: the files a change touches, read whole, with the lines it added marked so that
//! only hits on them are findings. What cannot be read is returned, never dropped.

use std::collections::BTreeSet;
use std::path::Path;

use debuggatha_core::env::Env;
use debuggatha_core::knowledge_system::ReviewPolicy;
use debuggatha_core::review_engine::FindingError;

use crate::control::Control;
use crate::detect::coverage::{SkipReason, SkippedFiles};
use crate::detect::{Registry, Run, SourceUnit, run_review_controlled};
use crate::diff::{ChangeKind, FileDiff, parse_unified_diff};
use crate::git::{self, GitError};
use crate::scan_scope::{MAX_REPO_FILE_BYTES, is_generated_content, is_ignored_path};

/// Why a diff review failed.
#[derive(Debug)]
pub enum DiffReviewError {
    /// Git failed.
    Git(GitError),
    /// A finding could not be built.
    Finding(FindingError),
}

impl std::fmt::Display for DiffReviewError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Git(e) => e.fmt(f),
            Self::Finding(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for DiffReviewError {}

/// The result of reviewing a change.
#[derive(Debug)]
pub struct DiffReview {
    /// The changes that were reviewed, as parsed.
    pub diff: Vec<FileDiff>,
    /// What the detectors produced on the files the change touched.
    pub run: Run,
    /// Changed files that were not reviewed, by reason (`Coverage::skipped_files`).
    pub skipped: Vec<SkippedFiles>,
    /// Files the change removes: nothing to review, and what the ledger held for them is resolved.
    pub deleted: Vec<String>,
    /// The revision the change was taken against.
    pub base: Option<String>,
}

/// Reviews what changed in the working tree against `base` (`HEAD` when `None`), including the
/// files git does not track yet.
///
/// # Errors
/// A ref that is not safe, a directory that is not a repository, or a finding the contract refuses.
pub fn review_diff(
    root: &Path,
    base: Option<&str>,
    policy: &ReviewPolicy,
    registry: &Registry,
    env: &dyn Env,
) -> Result<DiffReview, DiffReviewError> {
    review_diff_controlled(root, base, policy, registry, env, &Control::none())
}

/// [`review_diff`] that can be stopped; a stopped review has `run.cancelled` set.
///
/// # Errors
/// As [`review_diff`].
pub fn review_diff_controlled(
    root: &Path,
    base: Option<&str>,
    policy: &ReviewPolicy,
    registry: &Registry,
    env: &dyn Env,
    control: &Control,
) -> Result<DiffReview, DiffReviewError> {
    let changes = git::changes(root, base).map_err(DiffReviewError::Git)?;
    review_parsed(
        root,
        &changes.diff,
        &changes.untracked,
        changes.base,
        policy,
        registry,
        env,
        control,
    )
}

/// Reviews a unified diff that was handed over (from a pipe, a pull request) rather than read from
/// git: the files it names are read from the working tree, and only its added lines are findings.
///
/// # Errors
/// A finding the contract refuses.
pub fn review_diff_text(
    root: &Path,
    diff: &str,
    policy: &ReviewPolicy,
    registry: &Registry,
    env: &dyn Env,
) -> Result<DiffReview, DiffReviewError> {
    review_diff_text_controlled(root, diff, policy, registry, env, &Control::none())
}

/// [`review_diff_text`] that can be stopped; a stopped review has `run.cancelled` set.
///
/// # Errors
/// As [`review_diff_text`].
pub fn review_diff_text_controlled(
    root: &Path,
    diff: &str,
    policy: &ReviewPolicy,
    registry: &Registry,
    env: &dyn Env,
    control: &Control,
) -> Result<DiffReview, DiffReviewError> {
    review_parsed(root, diff, &[], None, policy, registry, env, control)
}

#[allow(clippy::too_many_arguments)] // one private step shared by the four entry points
fn review_parsed(
    root: &Path,
    diff_text: &str,
    untracked: &[String],
    base: Option<String>,
    policy: &ReviewPolicy,
    registry: &Registry,
    env: &dyn Env,
    control: &Control,
) -> Result<DiffReview, DiffReviewError> {
    let diff = parse_unified_diff(diff_text);
    let read = read_units(root, &diff, untracked);
    let run = run_review_controlled(policy, &read.units, None, registry, env, control)
        .map_err(DiffReviewError::Finding)?;
    Ok(DiffReview {
        diff,
        run,
        skipped: read.skipped,
        deleted: read.deleted,
        base,
    })
}

struct Read {
    units: Vec<SourceUnit>,
    skipped: Vec<SkippedFiles>,
    deleted: Vec<String>,
}

fn read_units(root: &Path, diff: &[FileDiff], untracked: &[String]) -> Read {
    let mut units = Vec::new();
    let mut skips: Vec<(SkipReason, String)> = Vec::new();
    let mut deleted = Vec::new();

    let wanted = diff
        .iter()
        .map(|f| (f.path.as_str(), Some(f)))
        .chain(untracked.iter().map(|p| (p.as_str(), None)));
    for (path, change) in wanted {
        if change.is_some_and(|c| c.kind == ChangeKind::Deleted) {
            deleted.push(path.to_owned());
            continue;
        }
        let changed_lines: Option<BTreeSet<u32>> =
            change.map(|c| c.added.iter().map(|a| a.line).collect());
        if changed_lines.as_ref().is_some_and(BTreeSet::is_empty)
            && !change.is_some_and(|c| c.binary)
        {
            continue; // only removals: nothing was added to review
        }
        if is_ignored_path(Some(root), path) {
            skips.push((SkipReason::Ignored, path.to_owned()));
        } else if change.is_some_and(|c| c.binary) {
            skips.push((SkipReason::Binary, path.to_owned()));
        } else {
            match read_text(root, path) {
                Ok(content) if is_generated_content(&content) => {
                    skips.push((SkipReason::Generated, path.to_owned()));
                }
                Ok(content) => units.push(SourceUnit {
                    file: path.to_owned(),
                    content,
                    changed_lines,
                }),
                Err(reason) => skips.push((reason, path.to_owned())),
            }
        }
    }
    deleted.sort();
    units.sort_by(|a, b| debuggatha_core::js::utf16_cmp(&a.file, &b.file));
    Read {
        units,
        skipped: group(skips),
        deleted,
    }
}

pub(crate) fn read_text(root: &Path, relative: &str) -> Result<String, SkipReason> {
    let full = root.join(relative);
    let real = full.canonicalize().map_err(|_| SkipReason::Unreadable)?;
    let inside = root.canonicalize().is_ok_and(|r| real.starts_with(r));
    if !inside {
        return Err(SkipReason::SymlinkOutsideRoot);
    }
    let meta = std::fs::metadata(&real).map_err(|_| SkipReason::Unreadable)?;
    if !meta.is_file() {
        return Err(SkipReason::Unreadable);
    }
    if meta.len() > MAX_REPO_FILE_BYTES {
        return Err(SkipReason::TooLarge);
    }
    let bytes = std::fs::read(&real).map_err(|_| SkipReason::Unreadable)?;
    String::from_utf8(bytes).map_err(|_| SkipReason::NotUtf8)
}

pub(crate) fn group(skips: Vec<(SkipReason, String)>) -> Vec<SkippedFiles> {
    let mut out: Vec<SkippedFiles> = Vec::new();
    for (reason, path) in skips {
        match out.iter_mut().find(|s| s.reason == reason) {
            Some(s) => {
                s.files += 1;
                if s.examples.len() < 5 {
                    s.examples.push(path);
                }
            }
            None => out.push(SkippedFiles {
                reason,
                files: 1,
                examples: vec![path],
            }),
        }
    }
    out
}
