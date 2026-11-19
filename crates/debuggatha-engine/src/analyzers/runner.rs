//! Runs analyzers as separate processes: no shell, the repository as working directory, a time
//! limit and a cap on what is read back. A tool that is missing, unwanted or that fails never
//! fails the review; it is reported in `runs` and the review goes on without it.

use std::collections::BTreeSet;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use super::types::{
    Adapter, AnalyzerFinding, AnalyzerOptions, AnalyzerRun, Mode, RepoFacts, RunContext, Status,
    Trust,
};
use crate::control::{Control, Stage};
use crate::scan_scope::{is_ignored_path, list_owned_files};

const DEFAULT_TIMEOUT_SECONDS: u64 = 300;
const VERSION_TIMEOUT: Duration = Duration::from_secs(30);
/// What one analyzer may print before the run is given up.
const MAX_OUTPUT_BYTES: usize = 64 * 1024 * 1024;
/// How long a pipe is waited for after its process ended, in case a child of it still holds it.
const PIPE_GRACE: Duration = Duration::from_secs(5);

/// The directories of `PATH`, in order.
#[must_use]
pub fn path_dirs() -> Vec<PathBuf> {
    std::env::var_os("PATH")
        .map(|p| {
            std::env::split_paths(&p)
                .filter(|d| !d.as_os_str().is_empty())
                .collect()
        })
        .unwrap_or_default()
}

fn is_executable(path: &Path) -> bool {
    let Ok(meta) = std::fs::metadata(path) else {
        return false;
    };
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        meta.is_file() && meta.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        meta.is_file()
    }
}

/// A repository-local install wins over one on `PATH`: it is the version the project pinned.
#[must_use]
pub fn find_executable(root: &Path, adapter: &Adapter, search: &[PathBuf]) -> Option<PathBuf> {
    adapter
        .local_binaries
        .iter()
        .map(|local| root.join(local))
        .chain(
            adapter
                .binary_names
                .iter()
                .flat_map(|name| search.iter().map(move |dir| dir.join(name))),
        )
        .find(|p| is_executable(p))
}

struct Captured {
    status: Option<i32>,
    stdout: String,
    stderr: String,
}

enum Failure {
    Cancelled,
    TimedOut,
    CouldNotStart,
    TooMuchOutput,
}

fn drain(mut pipe: impl Read + Send + 'static) -> mpsc::Receiver<(Vec<u8>, bool)> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let mut kept = Vec::new();
        let mut over = false;
        let mut buffer = vec![0_u8; 64 * 1024];
        while let Ok(n) = pipe.read(&mut buffer) {
            if n == 0 {
                break;
            }
            // Keep reading past the cap so the process never blocks on a full pipe.
            if kept.len() + n > MAX_OUTPUT_BYTES {
                over = true;
            } else {
                kept.extend_from_slice(&buffer[..n]);
            }
        }
        tx.send((kept, over)).ok();
    });
    rx
}

fn run_process(
    executable: &Path,
    args: &[String],
    cwd: &Path,
    timeout: Duration,
    control: Option<&Control>,
) -> Result<Captured, Failure> {
    let mut child = Command::new(executable)
        .args(args)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|_| Failure::CouldNotStart)?;
    let out = child.stdout.take().map(drain);
    let err = child.stderr.take().map(drain);

    let deadline = Instant::now() + timeout;
    let status = loop {
        if control.is_some_and(Control::cancelled) {
            child.kill().ok();
            child.wait().ok();
            return Err(Failure::Cancelled);
        }
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(10)),
            Ok(None) => {
                child.kill().ok();
                child.wait().ok();
                return Err(Failure::TimedOut);
            }
            Err(_) => return Err(Failure::CouldNotStart),
        }
    };
    let collect = |rx: Option<mpsc::Receiver<(Vec<u8>, bool)>>| {
        rx.and_then(|r| r.recv_timeout(PIPE_GRACE).ok())
            .unwrap_or_default()
    };
    let (stdout, over_out) = collect(out);
    let (stderr, over_err) = collect(err);
    if over_out || over_err {
        return Err(Failure::TooMuchOutput);
    }
    Ok(Captured {
        status: status.code(),
        stdout: String::from_utf8_lossy(&stdout).into_owned(),
        stderr: String::from_utf8_lossy(&stderr).into_owned(),
    })
}

/// The first `N.N` or `N.N.N` in the tool's output, with what follows it (`1.2.3-rc.1`).
fn version_in(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let digits = |from: usize| {
        bytes[from..]
            .iter()
            .take_while(|b| b.is_ascii_digit())
            .count()
    };
    (0..bytes.len()).find_map(|i| {
        let first = digits(i);
        if first == 0 || bytes.get(i + first) != Some(&b'.') {
            return None;
        }
        let second = digits(i + first + 1);
        if second == 0 {
            return None;
        }
        let rest = bytes[i + first + 1 + second..]
            .iter()
            .take_while(|b| b.is_ascii_alphanumeric() || matches!(**b, b'-' | b'+' | b'.' | b'_'))
            .count();
        Some(text[i..i + first + 1 + second + rest].to_owned())
    })
}

fn read_version(executable: &Path, adapter: &Adapter, root: &Path) -> Option<String> {
    let args: Vec<String> = adapter
        .prefix
        .iter()
        .chain(adapter.version_args)
        .map(|a| (*a).to_owned())
        .collect();
    let out = run_process(executable, &args, root, VERSION_TIMEOUT, None).ok()?;
    (out.status == Some(0))
        .then(|| version_in(&format!("{}\n{}", out.stdout, out.stderr)))
        .flatten()
}

/// Why an analyzer does not run, or `None` when it does.
fn gate(
    adapter: &Adapter,
    options: &AnalyzerOptions,
    applies: bool,
    installed: bool,
) -> Option<String> {
    let listed = |ids: &[String]| ids.iter().any(|id| id == adapter.id);
    if options.mode == Mode::Off {
        Some("analyzers are turned off".to_owned())
    } else if listed(&options.disable) {
        Some("disabled in the repository's configuration".to_owned())
    } else if !applies {
        Some("this repository has nothing it analyzes".to_owned())
    } else if !installed {
        Some("not installed".to_owned())
    } else if adapter.trust != Trust::Safe && !listed(&options.enable) {
        let why = match adapter.trust {
            Trust::RunsProjectCode => "it runs code the repository controls",
            _ => "it sends package names and versions to a service",
        };
        Some(format!("{why}; enable \"{}\" explicitly", adapter.id))
    } else {
        None
    }
}

fn repo_facts(root: &Path) -> RepoFacts {
    RepoFacts {
        root: root.to_path_buf(),
        files: list_owned_files(root),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InventoryEntry {
    pub id: &'static str,
    pub name: &'static str,
    pub license: &'static str,
    pub homepage: &'static str,
    pub covers: &'static str,
    pub trust: Trust,
    pub applies: bool,
    pub installed: bool,
    pub version: Option<String>,
    /// Whether a review would run it with the options given.
    pub will_run: bool,
    /// What stops it from running, when something does.
    pub reason: Option<String>,
}

/// What is installed and what a review would run.
#[must_use]
pub fn inventory(
    root: &Path,
    options: &AnalyzerOptions,
    search: &[PathBuf],
    adapters: &[&'static Adapter],
) -> Vec<InventoryEntry> {
    let facts = repo_facts(root);
    adapters
        .iter()
        .map(|adapter| {
            let executable = find_executable(root, adapter, search);
            let applies = (adapter.applies)(&facts);
            let reason = gate(adapter, options, applies, executable.is_some());
            InventoryEntry {
                id: adapter.id,
                name: adapter.name,
                license: adapter.license,
                homepage: adapter.homepage,
                covers: adapter.covers,
                trust: adapter.trust,
                applies,
                installed: executable.is_some(),
                version: executable.and_then(|e| read_version(&e, adapter, root)),
                will_run: reason.is_none(),
                reason,
            }
        })
        .collect()
}

#[derive(Debug)]
pub struct AnalyzerFindings {
    pub adapter: &'static Adapter,
    pub version: Option<String>,
    pub findings: Vec<AnalyzerFinding>,
}

#[derive(Debug, Default)]
pub struct AnalyzerRunResult {
    pub runs: Vec<AnalyzerRun>,
    pub results: Vec<AnalyzerFindings>,
}

pub struct RunInput<'a> {
    pub root: &'a Path,
    /// Repository-relative files under review, or `None` for the whole repository.
    pub files: Option<&'a [String]>,
    pub options: &'a AnalyzerOptions,
    /// Where executables are looked for after the repository's own: normally [`path_dirs`].
    pub search: &'a [PathBuf],
    pub adapters: &'a [&'static Adapter],
    /// Stops the run between tools and kills the one that is running.
    pub control: Option<&'a Control>,
}

fn elapsed_ms(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
}

/// The first line of what a failed tool printed, without the repository's path.
fn first_line(root: &Path, captured: &Captured) -> String {
    let text = if captured.stderr.trim().is_empty() {
        &captured.stdout
    } else {
        &captured.stderr
    };
    let line = text.trim().lines().find(|l| !l.is_empty()).unwrap_or("");
    let line = line.replace(&*root.to_string_lossy(), ".");
    line.chars().take(200).collect()
}

#[must_use]
pub fn run_analyzers(input: &RunInput<'_>) -> AnalyzerRunResult {
    let mut out = AnalyzerRunResult::default();
    if input.options.mode == Mode::Off {
        return out;
    }
    let facts = repo_facts(input.root);
    let timeout = Duration::from_secs(
        input
            .options
            .timeout_seconds
            .unwrap_or(DEFAULT_TIMEOUT_SECONDS),
    );

    for (done, adapter) in input.adapters.iter().enumerate() {
        if input.control.is_some_and(Control::cancelled) {
            return out;
        }
        if let Some(control) = input.control {
            control.report(Stage::Analyzers, done, input.adapters.len(), adapter.id);
        }
        let adapter: &'static Adapter = adapter;
        let run =
            |status, reason: Option<String>, findings, started: Option<Instant>| AnalyzerRun {
                id: adapter.id.to_owned(),
                name: adapter.name.to_owned(),
                license: adapter.license.to_owned(),
                trust: adapter.trust,
                status,
                reason,
                version: None,
                findings,
                duration_ms: started.map_or(0, elapsed_ms),
            };
        let executable = find_executable(input.root, adapter, input.search);
        let blocked = gate(
            adapter,
            input.options,
            (adapter.applies)(&facts),
            executable.is_some(),
        );
        let Some(executable) = executable.filter(|_| blocked.is_none()) else {
            out.runs.push(run(
                Status::Skipped,
                Some(blocked.unwrap_or_else(|| "not installed".to_owned())),
                0,
                None,
            ));
            continue;
        };

        // A tool that takes paths gets only the files under review; the others run whole and their
        // findings are filtered afterwards.
        let targets = match (input.files, adapter.file_filter) {
            (Some(files), Some(filter)) => {
                let targets: Vec<String> = files
                    .iter()
                    // A file the review names may be gone (a deletion): nothing is left to analyze.
                    .filter(|f| filter(f) && input.root.join(f).exists())
                    .map(|f| {
                        if f.starts_with('-') {
                            format!("./{f}")
                        } else {
                            f.clone()
                        }
                    })
                    .collect();
                if targets.is_empty() {
                    let reason = "no matching files in this review".to_owned();
                    out.runs.push(run(Status::Skipped, Some(reason), 0, None));
                    continue;
                }
                Some(targets)
            }
            _ => None,
        };
        let started = Instant::now();
        let version = read_version(&executable, adapter, input.root);
        match execute(&executable, adapter, input, targets.as_deref(), timeout) {
            Err(reason) => out
                .runs
                .push(run(Status::Failed, Some(reason), 0, Some(started))),
            Ok(findings) => {
                let mut finished = run(
                    Status::Ran,
                    None,
                    u32::try_from(findings.len()).unwrap_or(u32::MAX),
                    Some(started),
                );
                finished.version.clone_from(&version);
                out.runs.push(finished);
                out.results.push(AnalyzerFindings {
                    adapter,
                    version,
                    findings,
                });
            }
        }
    }
    out
}

/// One run of one tool: its findings in scope, or why the run failed.
fn execute(
    executable: &Path,
    adapter: &Adapter,
    input: &RunInput<'_>,
    targets: Option<&[String]>,
    timeout: Duration,
) -> Result<Vec<AnalyzerFinding>, String> {
    let invocation = (adapter.command)(&RunContext {
        root: input.root,
        files: targets,
    });
    let args: Vec<String> = adapter
        .prefix
        .iter()
        .map(|a| (*a).to_owned())
        .chain(invocation.args)
        .collect();
    let outcome = run_process(executable, &args, input.root, timeout, input.control);
    if let Some(directory) = &invocation.cleanup {
        std::fs::remove_dir_all(directory).ok();
    }
    let captured = outcome.map_err(|failure| match failure {
        Failure::Cancelled => "cancelled".to_owned(),
        Failure::TimedOut => format!("timed out after {}s", timeout.as_secs()),
        Failure::CouldNotStart => "could not start".to_owned(),
        Failure::TooMuchOutput => "printed more than it can be read".to_owned(),
    })?;
    if !captured
        .status
        .is_some_and(|code| adapter.ok_exit_codes.contains(&code))
    {
        let code = captured.status.map_or("?".to_owned(), |c| c.to_string());
        let detail = first_line(input.root, &captured);
        return Err(if detail.is_empty() {
            format!("exited with code {code}")
        } else {
            format!("exited with code {code}: {detail}")
        });
    }
    let parsed = (adapter.parse)(&captured.stdout, input.root)
        .map_err(|why| format!("output not understood: {why}"))?;
    let in_scope: Option<BTreeSet<&str>> = input
        .files
        .map(|files| files.iter().map(String::as_str).collect());
    Ok(parsed
        .into_iter()
        .filter(|f| {
            !is_ignored_path(Some(input.root), &f.file)
                && in_scope
                    .as_ref()
                    .is_none_or(|s| s.contains(f.file.as_str()))
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::version_in;

    #[test]
    fn the_version_is_the_first_dotted_number() {
        assert_eq!(
            version_in("gitleaks version 8.18.4\n").as_deref(),
            Some("8.18.4")
        );
        assert_eq!(
            version_in("tool v1.2.3-rc.1+b5 (abc)").as_deref(),
            Some("1.2.3-rc.1+b5")
        );
        assert_eq!(version_in("v12.0").as_deref(), Some("12.0"));
        assert_eq!(version_in("no number 7 here"), None);
        assert_eq!(version_in(""), None);
    }
}
