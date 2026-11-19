use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::path::Path;

use clap::{Args as ClapArgs, ValueEnum};
use debuggatha_engine::analyzers::path_dirs;
use debuggatha_engine::core::env::SystemEnv;
use debuggatha_engine::core::review_engine::{Finding, ReviewDepth, Severity, summarize_findings};
use debuggatha_engine::diff::{ChangeKind, parse_unified_diff};
use debuggatha_engine::git;
use debuggatha_engine::pipeline::{ReviewInput, Scope, SemanticInput, execute_review};
use debuggatha_engine::semantic_layer::{
    LocalRuntime, SemanticOptions, SemanticRun, connect_local,
};

use crate::commands::analyzers::AnalyzerArgs;
use crate::config::{self, Config, Loaded, glob_matches};
use crate::report::{self, ReportFormat, View, describe_runs};
use crate::{Context, client_message};

#[derive(Clone, Copy, ValueEnum)]
enum Runtime {
    Ollama,
    Lmstudio,
}

#[derive(ClapArgs)]
struct SemanticArgs {
    /// Also ask a model on this machine to read the changed code.
    #[arg(long = "semantic", value_enum)]
    runtime: Option<Runtime>,
    /// The model to ask (default: the first one the runtime has).
    #[arg(long = "semantic-model", requires = "runtime")]
    model: Option<String>,
    /// Where the runtime listens; only localhost is accepted.
    #[arg(long = "semantic-url", requires = "runtime")]
    url: Option<String>,
    /// Also attach the model's opinion to findings the detectors are unsure of.
    #[arg(long = "semantic-verify", requires = "runtime")]
    verify: bool,
}

#[derive(ClapArgs)]
#[allow(clippy::struct_excessive_bools)] // each is a switch of the command line
pub(crate) struct Args {
    /// Review the whole repository, even with uncommitted changes.
    #[arg(long, conflicts_with_all = ["files", "diff", "base", "changed_since"])]
    all: bool,
    /// A unified diff to review; "-" reads it from stdin.
    #[arg(long, conflicts_with_all = ["files", "base", "changed_since"])]
    diff: Option<String>,
    /// Review these whole files instead of the changes.
    #[arg(long, num_args = 1.., conflicts_with_all = ["base", "changed_since"])]
    files: Vec<String>,
    /// Review the changes since this git ref (for example origin/main...HEAD).
    #[arg(long)]
    base: Option<String>,
    /// Review whole files changed since this git ref, including new ones.
    #[arg(long, conflicts_with = "base")]
    changed_since: Option<String>,
    /// Fail only on findings on lines you changed; the rest are reported but do not count.
    #[arg(long)]
    fail_on_new: bool,
    /// Assert the review policy id a previous review reported.
    #[arg(long)]
    policy: Option<String>,
    /// Extra Review Packs on top of the detected ones.
    #[arg(long, num_args = 1..)]
    pack: Vec<String>,
    /// Lowest severity that makes the run exit with 2 (informational, low, medium, high,
    /// critical or none).
    #[arg(long, value_parser = clap::builder::PossibleValuesParser::new(["informational", "low", "medium", "high", "critical", "none"]))]
    fail_on: Option<String>,
    /// Write the report to a file instead of stdout.
    #[arg(long)]
    output: Option<std::path::PathBuf>,
    /// Print only the findings; no summary lines.
    #[arg(long)]
    quiet: bool,
    /// Do not record findings in .debuggatha/ledger.json.
    #[arg(long)]
    no_persist: bool,
    /// Also report findings the baseline accepted.
    #[arg(long)]
    include_baselined: bool,
    #[command(flatten)]
    analyzers: AnalyzerArgs,
    #[command(flatten)]
    semantic: SemanticArgs,
}

/// What a review looks at, or the reason there is nothing to look at.
enum Plan {
    Review(Scope),
    Nothing(String),
}

fn stdin_diff() -> Result<String, String> {
    let mut text = String::new();
    std::io::stdin()
        .read_to_string(&mut text)
        .map_err(|_| "The diff on stdin could not be read.".to_owned())?;
    if text.trim().is_empty() {
        return Err("--diff - expected a diff on stdin, but stdin was empty.".to_owned());
    }
    Ok(text)
}

fn plan(root: &Path, args: &Args) -> Result<Plan, String> {
    if args.all {
        return Ok(Plan::Review(Scope::Workspace));
    }
    if let Some(reference) = &args.changed_since {
        let changes = git::changes(root, Some(reference)).map_err(|e| client_message(&e))?;
        let mut files: Vec<String> = parse_unified_diff(&changes.diff)
            .into_iter()
            .map(|f| f.path)
            .collect();
        files.extend(changes.untracked);
        files.sort();
        files.dedup();
        return Ok(if files.is_empty() {
            Plan::Nothing(format!("No changes since {reference}."))
        } else {
            Plan::Review(Scope::Files(files))
        });
    }
    if !args.files.is_empty() {
        return Ok(Plan::Review(Scope::Files(args.files.clone())));
    }
    if let Some(diff) = &args.diff {
        let text = if diff == "-" {
            stdin_diff()?
        } else {
            diff.clone()
        };
        return Ok(Plan::Review(Scope::DiffText(text)));
    }
    if args.base.is_some() || root.join(".git").exists() {
        let changes = git::changes(root, args.base.as_deref()).map_err(|e| client_message(&e))?;
        if changes.diff.trim().is_empty() && changes.untracked.is_empty() {
            return match &args.base {
                Some(base) => Err(format!("No changes found against \"{base}\".")),
                None => Ok(Plan::Review(Scope::Workspace)),
            };
        }
        return Ok(Plan::Review(Scope::Diff {
            base: args.base.clone(),
        }));
    }
    Ok(Plan::Review(Scope::Workspace))
}

/// The lowest severity that fails, or `None` for `none`.
fn threshold(name: &str) -> Option<Severity> {
    match name {
        "critical" => Some(Severity::Critical),
        "high" => Some(Severity::High),
        "medium" => Some(Severity::Medium),
        "low" => Some(Severity::Low),
        "none" => None,
        _ => Some(Severity::Informational),
    }
}

/// Where the config's patterns start from: a review run inside `packages/api` still matches
/// `packages/api/**`.
fn config_prefix(root: &Path, loaded: &Loaded) -> String {
    loaded
        .root
        .as_deref()
        .and_then(|c| root.strip_prefix(c).ok())
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .unwrap_or_default()
}

fn from_config_root(prefix: &str, file: &str) -> String {
    if prefix.is_empty() {
        file.to_owned()
    } else {
        format!("{prefix}/{file}")
    }
}

fn first_file(finding: &Finding) -> &str {
    finding.locations.first().map_or("", |l| l.file.as_str())
}

/// The findings on lines added since `reference`, or in files that are new; a finding with no line
/// counts when its file changed.
fn on_changed_lines(
    root: &Path,
    reference: &str,
    findings: &[Finding],
) -> Result<Vec<bool>, String> {
    let base = (reference != "HEAD").then_some(reference);
    let changes = git::changes(root, base).map_err(|e| client_message(&e))?;
    let added: BTreeMap<String, BTreeSet<u32>> = parse_unified_diff(&changes.diff)
        .into_iter()
        .filter(|f| f.kind != ChangeKind::Deleted)
        .map(|f| (f.path, f.added.iter().map(|a| a.line).collect()))
        .collect();
    let fresh: BTreeSet<&str> = changes.untracked.iter().map(String::as_str).collect();
    Ok(findings
        .iter()
        .map(|f| {
            let file = first_file(f);
            if fresh.contains(file) {
                return true;
            }
            let Some(lines) = added.get(file) else {
                return false;
            };
            f.locations
                .first()
                .and_then(|l| l.lines)
                .is_none_or(|r| lines.contains(&r.start))
        })
        .collect())
}

/// A monorepo's config can leave paths out; what is left, and how many were.
fn leave_out_ignored(prefix: &str, loaded: &Loaded, findings: &[Finding]) -> (Vec<Finding>, usize) {
    let kept: Vec<Finding> = findings
        .iter()
        .filter(|f| {
            let file = first_file(f);
            file.is_empty()
                || !loaded
                    .config
                    .ignore
                    .iter()
                    .any(|g| glob_matches(g, &from_config_root(prefix, file)))
        })
        .cloned()
        .collect();
    let ignored = findings.len() - kept.len();
    (kept, ignored)
}

/// Does a counted finding reach its bar: the run's, or the one the config sets for its path?
fn exceeds_bar(
    prefix: &str,
    loaded: &Loaded,
    fail_on: &str,
    kept: &[Finding],
    counts: &[bool],
) -> bool {
    kept.iter().zip(counts).any(|(finding, counted)| {
        let path = from_config_root(prefix, first_file(finding));
        let bar = loaded
            .config
            .overrides
            .iter()
            .find(|(globs, _)| globs.iter().any(|g| glob_matches(g, &path)))
            .map_or(fail_on, |(_, f)| f.as_str());
        *counted && threshold(bar).is_some_and(|limit| finding.severity <= limit)
    })
}

fn semantic_notes(run: &SemanticRun) -> Vec<String> {
    let who = run
        .model
        .as_deref()
        .map_or_else(|| run.provider.clone(), |m| format!("{} {m}", run.provider));
    let mut lines = vec![format!(
        "Semantic pass ({who}): {} suspected {} raised, {} dropped for not matching the code{}.",
        run.detected,
        if run.detected == 1 { "issue" } else { "issues" },
        run.rejected,
        if run.verified > 0 {
            format!(", {} findings commented on", run.verified)
        } else {
            String::new()
        }
    )];
    lines.extend(run.notes.iter().map(|n| format!("Semantic pass: {n}")));
    lines
}

fn provider_from(
    args: &SemanticArgs,
) -> Result<Option<debuggatha_engine::semantic_layer::LocalProvider>, String> {
    args.runtime
        .map(|runtime| {
            let runtime = match runtime {
                Runtime::Ollama => LocalRuntime::Ollama,
                Runtime::Lmstudio => LocalRuntime::LmStudio,
            };
            connect_local(runtime, args.url.as_deref(), args.model.as_deref(), None)
        })
        .transpose()
}

fn emit(report: &str, args: &Args, format: ReportFormat) -> Result<(), String> {
    if let Some(file) = &args.output {
        std::fs::write(file, format!("{report}\n"))
            .map_err(|_| "The report file could not be written.".to_owned())?;
        if format == ReportFormat::Text && !args.quiet {
            println!("Report written to {}.", file.display());
        }
    } else if !report.is_empty() {
        println!("{report}");
    }
    Ok(())
}

/// `Ok(true)` when a counted finding reaches its bar.
pub(crate) fn run(context: &Context, args: &Args) -> Result<bool, String> {
    let root = context.root()?;
    let loaded = config::load(&root)?;
    let Config { ref packs, .. } = loaded.config;
    let format = context
        .report()
        .or(loaded.config.format)
        .unwrap_or(ReportFormat::Text);
    let fail_on = args
        .fail_on
        .clone()
        .or_else(|| loaded.config.fail_on.clone())
        .unwrap_or_else(|| "informational".to_owned());
    if !args.quiet {
        for warning in &loaded.warnings {
            eprintln!("debuggatha: {warning}");
        }
    }

    let scope = match plan(&root, args)? {
        Plan::Review(scope) => scope,
        Plan::Nothing(note) => {
            emit(&report::empty(format, &note), args, format)?;
            if format == ReportFormat::Text && !args.quiet {
                println!("{note}");
            }
            return Ok(false);
        }
    };
    let provider = provider_from(&args.semantic)?;
    let output = execute_review(
        &ReviewInput {
            root: &root,
            scope,
            depth: ReviewDepth::Full,
            pack_ids: packs.iter().chain(&args.pack).cloned().collect(),
            policy_id: args.policy.clone(),
            source_name: "debuggatha-cli",
            persist: !args.no_persist,
            include_baselined: args.include_baselined,
            analyzers: Some(args.analyzers.options(&loaded.config)),
            analyzer_search: path_dirs(),
            semantic: provider.as_ref().map(|provider| SemanticInput {
                provider,
                options: SemanticOptions {
                    verify: args.semantic.verify,
                    ..SemanticOptions::default()
                },
            }),
        },
        &SystemEnv,
    )
    .map_err(|e| client_message(&e))?;

    let prefix = config_prefix(&root, &loaded);
    let (kept, ignored) = leave_out_ignored(&prefix, &loaded, &output.result.findings);
    let summary = summarize_findings(
        &kept,
        output.result.summary.duration_ms,
        output.result.summary.applied_packs.clone(),
        output.result.summary.applied_policies.clone(),
    );

    // With --fail-on-new only what this change touched can fail the run.
    let counts = if args.fail_on_new {
        let reference = args
            .changed_since
            .as_deref()
            .or(args.base.as_deref())
            .unwrap_or("HEAD");
        on_changed_lines(&root, reference, &kept)?
    } else {
        vec![true; kept.len()]
    };
    let counted = counts.iter().filter(|c| **c).count();
    let outside = kept.len() - counted;
    let uncounted = (outside > 0).then(|| {
        format!(
            "{outside} more {} outside the lines this change touched and {} not affect the result.",
            if outside == 1 {
                "finding is"
            } else {
                "findings are"
            },
            if outside == 1 { "does" } else { "do" }
        )
    });

    let mut notes = describe_runs(&output.analyzers);
    if let Some(run) = &output.semantic {
        notes.extend(semantic_notes(run));
    }
    let view = View {
        output: &output,
        findings: &kept,
        summary: &summary,
        counted,
        notes,
        uncounted,
        ignored,
        quiet: args.quiet,
        persisted: !args.no_persist,
    };
    emit(&report::render(&view, format), args, format)?;

    let fails = exceeds_bar(&prefix, &loaded, &fail_on, &kept, &counts);
    Ok(fails)
}
