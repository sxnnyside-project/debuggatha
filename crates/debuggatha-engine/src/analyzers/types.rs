use std::path::{Path, PathBuf};

use debuggatha_core::review_engine::Severity;
use serde::{Deserialize, Serialize};

/// What running an analyzer can do to the machine, and so whether it may run without being asked.
/// Only `Safe` runs by default. The others need to be enabled by name by the person running
/// Debuggatha, never by a tool argument or a repository file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Trust {
    /// Reads source files only (a formatter, a secret scanner).
    Safe,
    /// Loads code the repository controls: a lint config that is JavaScript, a build script.
    RunsProjectCode,
    /// Sends data (package names and versions) to a service.
    Network,
}

/// One finding in a common shape, whatever the tool's own format was.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnalyzerFinding {
    pub rule_id: String,
    pub message: String,
    pub severity: Severity,
    pub category: String,
    /// Repository-relative, `/` separators.
    pub file: String,
    pub line: Option<u32>,
    pub column: Option<u32>,
    pub end_line: Option<u32>,
    pub end_column: Option<u32>,
    pub url: Option<String>,
    /// The line itself is a secret or holds one, so it is never copied into a report.
    pub sensitive: bool,
}

/// What the repository looks like, read once per review and shared by every adapter's `applies`.
pub struct RepoFacts {
    pub root: PathBuf,
    pub files: Vec<String>,
}

impl RepoFacts {
    #[must_use]
    pub fn any(&self, matches: impl Fn(&str) -> bool) -> bool {
        self.files.iter().any(|f| matches(f))
    }

    /// Some file of the repository has this base name.
    #[must_use]
    pub fn has_file_named(&self, names: &[&str]) -> bool {
        self.any(|f| names.contains(&f.rsplit('/').next().unwrap_or(f)))
    }
}

pub struct RunContext<'a> {
    pub root: &'a Path,
    /// Repository-relative files to look at, or `None` for the whole repository.
    pub files: Option<&'a [String]>,
}

pub struct Invocation {
    pub args: Vec<String>,
    /// A temporary directory to remove after the run (a generated config).
    pub cleanup: Option<PathBuf>,
}

#[derive(Debug)]
pub struct Adapter {
    /// Stable id, used to enable it and to name it in evidence.
    pub id: &'static str,
    pub name: &'static str,
    /// SPDX id of the tool's own license. Debuggatha runs the tool; it never bundles it.
    pub license: &'static str,
    pub homepage: &'static str,
    pub covers: &'static str,
    pub trust: Trust,
    /// Whether this repository uses what the tool analyzes; reads the file listing only.
    pub applies: fn(&RepoFacts) -> bool,
    /// Repository-local executables first (the version the project pinned), then names on `PATH`.
    pub local_binaries: &'static [&'static str],
    pub binary_names: &'static [&'static str],
    /// Arguments placed before the tool's own (`cargo` needs `clippy`).
    pub prefix: &'static [&'static str],
    pub version_args: &'static [&'static str],
    pub command: fn(&RunContext<'_>) -> Invocation,
    /// Exit codes that mean "ran, and may have found something". Anything else is a failure.
    pub ok_exit_codes: &'static [i32],
    pub parse: fn(&str, &Path) -> Result<Vec<AnalyzerFinding>, String>,
    /// Internal rule ids this tool answers for entirely, so ours would only repeat it.
    pub supersedes: &'static [&'static str],
    /// `(tool rule, internal rule)`: where the tool reports the same rule on the same line, its
    /// finding replaces ours (it is the repository's own linter, configured by the repository).
    pub equivalent_to: &'static [(&'static str, &'static str)],
    /// Which files of a review the tool is handed when the review has a scope.
    pub file_filter: Option<fn(&str) -> bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Ran,
    Skipped,
    Failed,
}

/// What happened to one analyzer in one review, reported whether or not it found anything.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalyzerRun {
    pub id: String,
    pub name: String,
    pub license: String,
    pub trust: Trust,
    pub status: Status,
    /// Why it did not run or failed; absent when it ran.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    pub findings: u32,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    /// Runs the safe analyzers that are installed and apply.
    #[default]
    Auto,
    Off,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalyzerOptions {
    #[serde(default)]
    pub mode: Mode,
    /// Analyzers to run although they load project code or use the network: the person's choice.
    #[serde(default)]
    pub enable: Vec<String>,
    /// Analyzers to leave out (a repository's own choice; it can only narrow what runs).
    #[serde(default)]
    pub disable: Vec<String>,
    /// Seconds one analyzer may run before it is stopped; 300 when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_seconds: Option<u64>,
}
