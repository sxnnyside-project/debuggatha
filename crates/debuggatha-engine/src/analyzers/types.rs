//! The shapes of an external analyzer: adapter, run, finding and options.

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
    /// The tool's own rule id.
    pub rule_id: String,
    /// What the tool says.
    pub message: String,
    /// Severity, mapped from the tool's.
    pub severity: Severity,
    /// Category, mapped from the tool's.
    pub category: String,
    /// Repository-relative, `/` separators.
    pub file: String,
    /// First line, 1-based.
    pub line: Option<u32>,
    /// First column, 1-based.
    pub column: Option<u32>,
    /// Last line.
    pub end_line: Option<u32>,
    /// Column after the last one.
    pub end_column: Option<u32>,
    /// A page that explains the rule.
    pub url: Option<String>,
    /// The line itself is a secret or holds one, so it is never copied into a report.
    pub sensitive: bool,
}

/// What the repository looks like, read once per review and shared by every adapter's `applies`.
pub struct RepoFacts {
    /// The repository root.
    pub root: PathBuf,
    /// Every file of the repository, relative to the root.
    pub files: Vec<String>,
}

impl RepoFacts {
    /// Whether any file matches.
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

/// What an adapter needs to build the command of one run.
pub struct RunContext<'a> {
    /// The repository root.
    pub root: &'a Path,
    /// Repository-relative files to look at, or `None` for the whole repository.
    pub files: Option<&'a [String]>,
}

/// The command an analyzer is run with.
pub struct Invocation {
    /// The arguments, after the program.
    pub args: Vec<String>,
    /// A temporary directory to remove after the run (a generated config).
    pub cleanup: Option<PathBuf>,
}

/// How to run one external analyzer and read what it prints. Debuggatha runs it as a separate process and never bundles it.
#[derive(Debug)]
pub struct Adapter {
    /// Stable id, used to enable it and to name it in evidence.
    pub id: &'static str,
    /// Name shown to people.
    pub name: &'static str,
    /// SPDX id of the tool's own license. Debuggatha runs the tool; it never bundles it.
    pub license: &'static str,
    /// The tool's home page.
    pub homepage: &'static str,
    /// What the tool analyzes, in a few words.
    pub covers: &'static str,
    /// What running it does to the machine.
    pub trust: Trust,
    /// Whether this repository uses what the tool analyzes; reads the file listing only.
    pub applies: fn(&RepoFacts) -> bool,
    /// Repository-local executables first (the version the project pinned), then names on `PATH`.
    pub local_binaries: &'static [&'static str],
    /// Executable names to look for on the `PATH`.
    pub binary_names: &'static [&'static str],
    /// Arguments placed before the tool's own (`cargo` needs `clippy`).
    pub prefix: &'static [&'static str],
    /// Arguments that make the tool print its version.
    pub version_args: &'static [&'static str],
    /// Builds the command for one run.
    pub command: fn(&RunContext<'_>) -> Invocation,
    /// Exit codes that mean "ran, and may have found something". Anything else is a failure.
    pub ok_exit_codes: &'static [i32],
    /// Turns the tool's output into findings; an error string when the output cannot be read.
    pub parse: fn(&str, &Path) -> Result<Vec<AnalyzerFinding>, String>,
    /// Internal rule ids this tool answers for entirely, so ours would only repeat it.
    pub supersedes: &'static [&'static str],
    /// `(tool rule, internal rule)`: where the tool reports the same rule on the same line, its
    /// finding replaces ours (it is the repository's own linter, configured by the repository).
    pub equivalent_to: &'static [(&'static str, &'static str)],
    /// Which files of a review the tool is handed when the review has a scope.
    pub file_filter: Option<fn(&str) -> bool>,
}

/// What became of one analyzer in one review.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    /// It ran.
    Ran,
    /// It was not run.
    Skipped,
    /// It ran and failed.
    Failed,
}

/// What happened to one analyzer in one review, reported whether or not it found anything.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalyzerRun {
    /// The analyzer's id.
    pub id: String,
    /// Name shown to people.
    pub name: String,
    /// The tool's license (SPDX).
    pub license: String,
    /// What running it does to the machine.
    pub trust: Trust,
    /// What became of it.
    pub status: Status,
    /// Why it did not run or failed; absent when it ran.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// The tool's version, when it reports one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// How many findings it produced.
    pub findings: u32,
    /// How long it took, in milliseconds.
    pub duration_ms: u64,
}

/// Whether analyzers run at all.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    /// Runs the safe analyzers that are installed and apply.
    #[default]
    Auto,
    /// Run none.
    Off,
}

/// What the person allowed analyzers to do; a request can never widen it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalyzerOptions {
    /// Whether analyzers run.
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
