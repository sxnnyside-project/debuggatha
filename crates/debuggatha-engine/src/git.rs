//! What git says about the working tree. Arguments go to git as they are, never through a shell,
//! and a ref that could be read as an option is refused before any process starts.

use std::fmt;
use std::path::Path;
use std::process::{Command, Stdio};

/// Why a git call was refused or failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitError {
    /// A ref that could be read as an option.
    UnsafeRef,
    /// The directory is not a git repository.
    NotARepository,
    /// Git ran and failed (unknown ref, corrupt repository); its message is not passed on, since
    /// it names paths.
    Failed,
}

impl fmt::Display for GitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::UnsafeRef => "The git ref is not a branch, tag, commit or range.",
            Self::NotARepository => {
                "This is not a git repository, so there is no record of what changed."
            }
            Self::Failed => "git could not produce the changes against that ref.",
        })
    }
}

impl std::error::Error for GitError {}

/// A branch, tag, commit or range (`a...b`); never starts with `-`, so git cannot read it as an
/// option.
#[must_use]
pub fn is_safe_ref(reference: &str) -> bool {
    let mut chars = reference.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '@' | '{' | '}'))
        && chars.all(|c| {
            c.is_ascii_alphanumeric()
                || matches!(c, '_' | '.' | '/' | '@' | '{' | '}' | '^' | '~' | ':' | '-')
        })
}

/// What changed in the working tree against a base.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Changes {
    /// The ref the diff is against; `None` in a repository with no commit yet.
    pub base: Option<String>,
    /// Unified diff of tracked files, paths relative to `root`.
    pub diff: String,
    /// Files git does not track yet and does not ignore: a diff cannot show them.
    pub untracked: Vec<String>,
}

fn git(root: &Path, args: &[&str]) -> Result<Vec<u8>, GitError> {
    let out = Command::new("git")
        .args(args)
        .current_dir(root)
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env_remove("GIT_EXTERNAL_DIFF")
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .map_err(|_| GitError::Failed)?;
    if out.status.success() {
        Ok(out.stdout)
    } else {
        Err(GitError::Failed)
    }
}

/// # Errors
/// `UnsafeRef` before git runs, `NotARepository` outside a work tree, `Failed` when git does.
pub fn changes(root: &Path, base: Option<&str>) -> Result<Changes, GitError> {
    if base.is_some_and(|b| !is_safe_ref(b)) {
        return Err(GitError::UnsafeRef);
    }
    if git(root, &["rev-parse", "--is-inside-work-tree"]).is_err() {
        return Err(GitError::NotARepository);
    }
    let has_head = git(root, &["rev-parse", "--verify", "--quiet", "HEAD"]).is_ok();
    let reference = base.or(has_head.then_some("HEAD"));

    let (diff, mut untracked) = match reference {
        Some(r) => (
            git(
                root,
                &[
                    "diff",
                    "--no-color",
                    "--no-ext-diff",
                    "--no-textconv",
                    "--no-renames",
                    "--relative",
                    r,
                    "--",
                ],
            )?,
            Vec::new(),
        ),
        // No commit yet: every staged file is new.
        None => (
            Vec::new(),
            nul_list(&git(root, &["ls-files", "-z", "--cached"])?),
        ),
    };
    untracked.extend(nul_list(&git(
        root,
        &["ls-files", "-z", "--others", "--exclude-standard"],
    )?));
    untracked.sort();
    untracked.dedup();
    Ok(Changes {
        base: reference.map(str::to_owned),
        diff: String::from_utf8_lossy(&diff).into_owned(),
        untracked,
    })
}

fn nul_list(bytes: &[u8]) -> Vec<String> {
    String::from_utf8_lossy(bytes)
        .split('\0')
        .filter(|p| !p.is_empty())
        .map(str::to_owned)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refs_that_git_could_read_as_options_are_refused() {
        for ok in [
            "main",
            "HEAD~2",
            "origin/main...HEAD",
            "v1.2.3",
            "@{u}",
            "a1b2c3",
        ] {
            assert!(is_safe_ref(ok), "{ok}");
        }
        for bad in [
            "",
            "-p",
            "--output=/tmp/x",
            "a b",
            "a;rm -rf /",
            "$(id)",
            "a`id`",
            "a\nb",
            "a|b",
        ] {
            assert!(!is_safe_ref(bad), "{bad:?}");
        }
    }
}
