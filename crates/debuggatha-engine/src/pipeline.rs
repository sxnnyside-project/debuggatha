//! One review, from the repository to the ledger: context, policy, detectors, memory, baseline and
//! ledger. The three scopes differ in which files are read and nothing else.

use std::fmt;
use std::path::{Path, PathBuf};

use debuggatha_core::env::Env;
use debuggatha_core::findings_ledger::{
    Ledger, LedgerError, MatchOutcome, SyncReport, SyncScope, association_for,
    default_finding_matcher, load_ledger, save_ledger, snapshot_ref_for, synchronize_review_result,
};
use debuggatha_core::knowledge_system::{
    CapabilityRegistry, PolicyError, PolicyRequest, ResolutionContext, ReviewPolicy, resolve_policy,
};
use debuggatha_core::repository_intelligence::{
    RepositoryContext, ScanError, build_repository_context,
};
use debuggatha_core::repository_memory::{
    SuppressedFinding, filter_suppressed_findings, load_memory_store,
};
use debuggatha_core::review_engine::{
    CreateReviewRequestInput, CreateReviewSessionInput, EmptyFileScope, ExecutionPatch, Finding,
    FindingError, IllegalTransition, ReviewDepth, ReviewLifecycleStatus, ReviewResult, ReviewScope,
    ReviewSession, ReviewSource, SourceKind, create_review_request, create_review_session,
    summarize_findings, transition_session,
};
use debuggatha_core::stored::StoreError;

use crate::analyzers::{
    ADAPTERS, AnalyzerOptions, AnalyzerRun, Mode, RunInput, Status, analyzer_ids,
    merge_analyzer_findings, run_analyzers,
};
use crate::baseline::{
    Baseline, apply_baseline, create_baseline_from, load_baseline, save_baseline,
};
use crate::control::{Control, Stage};
use crate::detect::coverage::{Coverage, SkipReason, SkippedFiles};
use crate::detect::{Registry, RepoFile, SourceUnit, default_registry, run_review_controlled};
use crate::diff::FileDiff;
use crate::diff_review::{
    DiffReviewError, group, read_text, review_diff_controlled, review_diff_text_controlled,
};
use crate::packs::builtin_packs;
use crate::scan_scope::{is_generated_content, list_owned_files, list_repo_files};
use crate::semantic_layer::{
    Refine, Scope as SemanticScope, SemanticOptions, SemanticProvider, SemanticRun, refine_findings,
};
use crate::syntax::{InlineSuppression, Language};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Scope {
    /// Every file of the project that a parser or a detector reads.
    Workspace,
    /// These files, relative to the repository or absolute inside it.
    Files(Vec<String>),
    /// What changed against `base` (`HEAD` when `None`), added lines only.
    Diff { base: Option<String> },
    /// A unified diff handed over by the caller; the files it names are read from the tree.
    DiffText(String),
}

pub struct ReviewInput<'a> {
    pub root: &'a Path,
    pub scope: Scope,
    pub depth: ReviewDepth,
    /// Packs asked for by name; every other registered pack follows them.
    pub pack_ids: Vec<String>,
    pub policy_id: Option<String>,
    /// Who ran the review, recorded in the session ("debuggatha-cli").
    pub source_name: &'a str,
    /// Write the ledger back. `false` reads and reports only.
    pub persist: bool,
    /// Report what the baseline accepted too.
    pub include_baselined: bool,
    /// External analyzers to run beside the detectors; `None` runs none, so a library call stays
    /// deterministic. Only the person running Debuggatha builds this, never a repository file.
    pub analyzers: Option<AnalyzerOptions>,
    /// Where analyzer executables are looked for after the repository's own (see `path_dirs`).
    pub analyzer_search: Vec<PathBuf>,
    /// A model to read the changed code; `None` leaves the review without one.
    pub semantic: Option<SemanticInput<'a>>,
}

/// The person's choice to let a model look: which one, and what it may do. Code goes only where
/// the provider sends it; the local providers accept `localhost` alone.
pub struct SemanticInput<'a> {
    pub provider: &'a dyn SemanticProvider,
    pub options: SemanticOptions,
}

#[derive(Debug)]
pub enum PipelineError {
    Scan(ScanError),
    Request(EmptyFileScope),
    Policy(PolicyError),
    Session(IllegalTransition),
    Diff(DiffReviewError),
    Finding(FindingError),
    Store(StoreError),
    Ledger(LedgerError),
    /// The caller asked to stop; nothing was written.
    Cancelled,
}

impl fmt::Display for PipelineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Scan(e) => e.fmt(f),
            Self::Request(e) => e.fmt(f),
            Self::Policy(e) => e.fmt(f),
            Self::Session(e) => e.fmt(f),
            Self::Diff(e) => e.fmt(f),
            Self::Finding(e) => e.fmt(f),
            Self::Store(e) => e.fmt(f),
            Self::Ledger(e) => e.fmt(f),
            Self::Cancelled => f.write_str("The review was cancelled."),
        }
    }
}

impl std::error::Error for PipelineError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(match self {
            Self::Cancelled => return None,
            Self::Scan(e) => e,
            Self::Request(e) => e,
            Self::Policy(e) => e,
            Self::Session(e) => e,
            Self::Diff(e) => e,
            Self::Finding(e) => e,
            Self::Store(e) => e,
            Self::Ledger(e) => e,
        })
    }
}

/// A finding described enough to talk about without fetching it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FindingRef {
    pub ledger_id: String,
    pub title: String,
    pub file: String,
    pub line: Option<u32>,
    pub rule_id: Option<String>,
    pub severity: debuggatha_core::review_engine::Severity,
}

/// What this review changed in the ledger's picture of the repository. `introduced` and `reopened`
/// leave out what the baseline set aside.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReviewChanges {
    pub introduced: Vec<FindingRef>,
    pub fixed: Vec<FindingRef>,
    pub reopened: Vec<FindingRef>,
}

#[derive(Debug)]
pub struct ReviewOutput {
    /// What the review reports: everything found minus what memory, an inline comment or the
    /// baseline set aside.
    pub result: ReviewResult,
    pub policy: ReviewPolicy,
    pub coverage: Coverage,
    pub sync: SyncReport,
    pub changes: ReviewChanges,
    /// Findings an inline `debuggatha-ignore` comment hid.
    pub suppressed_inline: Vec<InlineSuppression>,
    /// Findings an active memory item covers: reported, never silently dropped.
    pub suppressed_by_memory: Vec<SuppressedFinding>,
    pub baselined: usize,
    /// Every analyzer considered: which ran, which did not and why. Empty when none were asked for.
    pub analyzers: Vec<AnalyzerRun>,
    /// What the model did; `None` when none was asked for.
    pub semantic: Option<SemanticRun>,
    /// Files a diff removes; the ledger resolves what it held for them.
    pub deleted: Vec<String>,
}

struct Read {
    units: Vec<SourceUnit>,
    skipped: Vec<SkippedFiles>,
}

/// Records every finding the repository has today as accepted, so later reviews report only what is
/// new. Reads the whole repository, ignores any baseline already there, and writes only
/// `.debuggatha/baseline.json`, never the ledger. It must run the same analyzers and packs the
/// reviews will, or every finding of one they add later would look new.
///
/// # Errors
/// As [`execute_review`], and the baseline file cannot be written.
pub fn create_baseline(
    input: &ReviewInput<'_>,
    env: &dyn Env,
) -> Result<(Baseline, Vec<AnalyzerRun>), PipelineError> {
    let output = execute_review(
        &ReviewInput {
            root: input.root,
            scope: Scope::Workspace,
            depth: ReviewDepth::Full,
            pack_ids: input.pack_ids.clone(),
            policy_id: None,
            source_name: input.source_name,
            persist: false,
            include_baselined: true,
            analyzers: input.analyzers.clone(),
            analyzer_search: input.analyzer_search.clone(),
            semantic: None,
        },
        env,
    )?;
    let baseline = create_baseline_from(&output.result.findings, env);
    save_baseline(input.root, &baseline).map_err(PipelineError::Store)?;
    Ok((baseline, output.analyzers))
}

/// # Errors
/// The repository cannot be scanned, a pack or policy cannot be resolved, a stored file cannot be
/// read or written, git refuses the base, or a finding breaks its contract.
pub fn execute_review(
    input: &ReviewInput<'_>,
    env: &dyn Env,
) -> Result<ReviewOutput, PipelineError> {
    execute_review_with(input, env, &Control::none())
}

/// [`execute_review`] that can be stopped and that reports its progress. A stopped review returns
/// `Cancelled` and has written nothing: the ledger sync, the last step, either completes or never
/// starts.
///
/// # Errors
/// As [`execute_review`], and `Cancelled`.
pub fn execute_review_with(
    input: &ReviewInput<'_>,
    env: &dyn Env,
    control: &Control,
) -> Result<ReviewOutput, PipelineError> {
    let stopped = || control.cancelled().then_some(PipelineError::Cancelled);
    control.report(Stage::Orient, 0, 1, "");
    let (context, policy, requested) =
        assemble_policy(input.root, &input.pack_ids, input.policy_id.as_deref(), env)?;
    let session = open_session(input, &context, &policy, requested, env)?;
    if let Some(stop) = stopped() {
        return Err(stop);
    }

    let detectors = default_registry();
    let found = detect(input, &policy, &detectors, env, control)?;
    if let Some(stop) = stopped() {
        return Err(stop);
    }
    control.report(Stage::Sync, 0, 1, "");

    let session = transition_session(
        session,
        ReviewLifecycleStatus::Completed,
        ExecutionPatch {
            completed_at: Some(env.now()),
            ..ExecutionPatch::default()
        },
    )
    .map_err(PipelineError::Session)?;

    let memory = load_memory_store(input.root, env).map_err(PipelineError::Store)?;
    let filtered = filter_suppressed_findings(&found.findings, &memory);
    let tracked = ReviewResult {
        session_id: session.id.clone(),
        summary: summarize_findings(
            &filtered.findings,
            None,
            session.selected_packs.clone(),
            session.selected_policy.iter().cloned().collect(),
        ),
        findings: filtered.findings.clone(),
        recommendations: Vec::new(),
        execution: session.execution.clone(),
    };

    let (reported, baselined) = report_new(input, &filtered.findings)?;

    let ledger = load_ledger(input.root, env).map_err(PipelineError::Store)?;
    let snapshot = snapshot_ref_for(
        &context.generated_at,
        &serde_json::to_value(&context.stack).unwrap_or_default(),
        &serde_json::to_value(&context.dependencies).unwrap_or_default(),
        &serde_json::to_value(&context.documentation).unwrap_or_default(),
        &serde_json::to_value(&context.criteria).unwrap_or_default(),
    );
    let sync_scope = sync_scope(input.root, &input.scope, &filtered.findings, &found.deleted);
    let (updated, sync) = synchronize_review_result(
        &ledger,
        &tracked,
        &association_for(&tracked, snapshot),
        &sync_scope,
        env,
    )
    .map_err(PipelineError::Ledger)?;
    let (updated, sync) = keep_unanalyzed(&ledger, updated, sync, &found.analyzers);
    if input.persist {
        save_ledger(&updated).map_err(PipelineError::Store)?;
    }

    let changes = changes_of(&updated, &sync, &reported);
    let result = ReviewResult {
        summary: summarize_findings(
            &reported,
            None,
            tracked.summary.applied_packs.clone(),
            tracked.summary.applied_policies.clone(),
        ),
        findings: reported,
        ..tracked
    };
    let coverage = found
        .run
        .coverage(&policy, &detectors, found.skipped.clone(), false);

    Ok(ReviewOutput {
        result,
        policy,
        coverage,
        sync,
        changes,
        suppressed_inline: found.run.suppressed.clone(),
        suppressed_by_memory: filtered.suppressed,
        baselined,
        analyzers: found.analyzers,
        semantic: found.semantic,
        deleted: found.deleted,
    })
}

/// What the baseline leaves to report, and how many findings it accepted.
fn report_new(
    input: &ReviewInput<'_>,
    findings: &[Finding],
) -> Result<(Vec<Finding>, usize), PipelineError> {
    let baseline = if input.include_baselined {
        None
    } else {
        load_baseline(input.root).map_err(PipelineError::Store)?
    };
    Ok(match baseline {
        Some(b) => {
            let applied = apply_baseline(findings, &b);
            (
                applied.fresh.into_iter().cloned().collect(),
                applied.baselined.len(),
            )
        }
        None => (findings.to_vec(), 0),
    })
}

/// The repository's context and the Review Policy a review of it would assemble: the packs asked
/// for first, then every registered pack, each rule kept only where the repository's detected
/// capabilities match. Also the pack ids requested, in order.
///
/// # Errors
/// The repository cannot be scanned, a pack cannot be resolved, or `policy_id` no longer matches.
pub fn assemble_policy(
    root: &Path,
    pack_ids: &[String],
    policy_id: Option<&str>,
    env: &dyn Env,
) -> Result<(RepositoryContext, ReviewPolicy, Vec<String>), PipelineError> {
    let context = build_repository_context(root, env, None).map_err(PipelineError::Scan)?;
    let registry = CapabilityRegistry::new(builtin_packs(), vec![]);
    let requested = requested_pack_ids(&registry, pack_ids);
    let policy = resolve_policy(
        &ResolutionContext::from_context(&context),
        &PolicyRequest {
            requested_pack_ids: requested.clone(),
            requested_policy_id: policy_id.map(str::to_owned),
        },
        &registry,
    )
    .map_err(PipelineError::Policy)?;
    Ok((context, policy, requested))
}

/// The request and the session, moved to `running`.
fn open_session(
    input: &ReviewInput<'_>,
    context: &RepositoryContext,
    policy: &ReviewPolicy,
    requested: Vec<String>,
    env: &dyn Env,
) -> Result<ReviewSession, PipelineError> {
    let scope = match &input.scope {
        Scope::Workspace => ReviewScope::Workspace,
        Scope::Files(paths) => ReviewScope::Files {
            paths: paths.clone(),
        },
        Scope::Diff { base } => ReviewScope::Diff { base: base.clone() },
        Scope::DiffText(_) => ReviewScope::Diff { base: None },
    };
    let mut request_input = CreateReviewRequestInput::new(scope, input.depth);
    request_input.requested_pack_ids = requested;
    request_input
        .requested_policy_id
        .clone_from(&input.policy_id);
    let request = create_review_request(request_input, env).map_err(PipelineError::Request)?;

    let session = create_review_session(
        CreateReviewSessionInput {
            id: None,
            created_at: None,
            request,
            repository_context: context.clone(),
            selected_policy: Some(debuggatha_core::review_engine::PolicyReference {
                id: policy.id.clone(),
            }),
            selected_packs: policy.pack_refs.clone(),
        },
        env,
    );
    let session = transition_session(
        session,
        ReviewLifecycleStatus::Prepared,
        ExecutionPatch::default(),
    )
    .map_err(PipelineError::Session)?;
    transition_session(
        session,
        ReviewLifecycleStatus::Running,
        ExecutionPatch {
            source: Some(ReviewSource {
                kind: SourceKind::DeterministicAnalyzer,
                name: input.source_name.to_owned(),
            }),
            started_at: Some(env.now()),
            ..ExecutionPatch::default()
        },
    )
    .map_err(PipelineError::Session)
}

/// An analyzer or a model that did not run says nothing about its earlier findings, so their
/// absence is not read as "fixed": entries the sync resolved for that reason go back as they were.
fn keep_unanalyzed(
    before: &Ledger,
    mut updated: Ledger,
    mut sync: SyncReport,
    runs: &[AnalyzerRun],
) -> (Ledger, SyncReport) {
    let known = analyzer_ids();
    let ran: Vec<&str> = runs
        .iter()
        .filter(|r| r.status == Status::Ran)
        .map(|r| r.id.as_str())
        .collect();
    let restore: Vec<String> = sync
        .auto_resolved_entry_ids
        .iter()
        .filter(|id| {
            let tool = before
                .entries
                .iter()
                .find(|e| &e.id == *id)
                .and_then(|e| e.fingerprint.rule_id.as_deref())
                .and_then(|rule| rule.split(':').next());
            tool.is_some_and(|t| t == "semantic" || (known.contains(t) && !ran.contains(&t)))
        })
        .cloned()
        .collect();
    for id in &restore {
        let original = before.entries.iter().find(|e| &e.id == id);
        if let (Some(original), Some(slot)) =
            (original, updated.entries.iter_mut().find(|e| &e.id == id))
        {
            *slot = original.clone();
        }
    }
    sync.auto_resolved_entry_ids
        .retain(|id| !restore.contains(id));
    (updated, sync)
}

struct Found {
    run: crate::detect::Run,
    diff: Vec<FileDiff>,
    semantic: Option<SemanticRun>,
    analyzers: Vec<AnalyzerRun>,
    findings: Vec<Finding>,
    skipped: Vec<SkippedFiles>,
    deleted: Vec<String>,
}

fn detect(
    input: &ReviewInput<'_>,
    policy: &ReviewPolicy,
    registry: &Registry,
    env: &dyn Env,
    control: &Control,
) -> Result<Found, PipelineError> {
    let mut found = detect_builtin(input, policy, registry, env, control)?;
    if found.run.cancelled || control.cancelled() {
        return Err(PipelineError::Cancelled);
    }
    if let Some(options) = input.analyzers.as_ref().filter(|o| o.mode != Mode::Off) {
        add_analyzers(input, policy, options, &mut found, env, control)?;
        if control.cancelled() {
            return Err(PipelineError::Cancelled);
        }
    }
    if let Some(semantic) = &input.semantic {
        control.report(Stage::Semantic, 0, 1, "");
        refine(input, policy, semantic, &mut found, env)?;
    }
    Ok(found)
}

fn add_analyzers(
    input: &ReviewInput<'_>,
    policy: &ReviewPolicy,
    options: &AnalyzerOptions,
    found: &mut Found,
    env: &dyn Env,
    control: &Control,
) -> Result<(), PipelineError> {
    let files = analyzed_files(input, found);
    let outcome = run_analyzers(&RunInput {
        root: input.root,
        files: files.as_deref(),
        options,
        search: &input.analyzer_search,
        adapters: ADAPTERS,
        control: Some(control),
    });
    found.findings = merge_analyzer_findings(
        std::mem::take(&mut found.findings),
        &outcome.results,
        input.root,
        &policy.id,
        env,
    )
    .map_err(PipelineError::Finding)?;
    found.analyzers = outcome.runs;
    Ok(())
}

/// The model reads what the detectors and analyzers left; it adds evidence and suspicions, and
/// never closes, hides or lowers a finding.
fn refine(
    input: &ReviewInput<'_>,
    policy: &ReviewPolicy,
    semantic: &SemanticInput<'_>,
    found: &mut Found,
    env: &dyn Env,
) -> Result<(), PipelineError> {
    let files: Vec<String> = match &input.scope {
        Scope::Files(paths) => paths
            .iter()
            .map(|p| to_repo_relative(input.root, p))
            .collect(),
        _ => Vec::new(),
    };
    let scope = match &input.scope {
        Scope::Workspace => SemanticScope::Workspace,
        Scope::Files(_) => SemanticScope::Files(&files),
        Scope::Diff { .. } | Scope::DiffText(_) => SemanticScope::Diff(&found.diff),
    };
    let (findings, run) = refine_findings(Refine {
        root: input.root,
        scope,
        findings: std::mem::take(&mut found.findings),
        policy_id: &policy.id,
        options: &semantic.options,
        provider: semantic.provider,
        env,
    })
    .map_err(PipelineError::Finding)?;
    found.findings = findings;
    found.semantic = Some(run);
    Ok(())
}

/// The files the analyzers are limited to; `None` is the whole repository.
fn analyzed_files(input: &ReviewInput<'_>, found: &Found) -> Option<Vec<String>> {
    match &input.scope {
        Scope::Workspace => None,
        Scope::Files(paths) => Some(
            paths
                .iter()
                .map(|p| to_repo_relative(input.root, p))
                .collect(),
        ),
        Scope::Diff { .. } | Scope::DiffText(_) => {
            Some(found.run.files.iter().map(|f| f.file.clone()).collect())
        }
    }
}

fn detect_builtin(
    input: &ReviewInput<'_>,
    policy: &ReviewPolicy,
    registry: &Registry,
    env: &dyn Env,
    control: &Control,
) -> Result<Found, PipelineError> {
    let changed = match &input.scope {
        Scope::Diff { base } => Some(review_diff_controlled(
            input.root,
            base.as_deref(),
            policy,
            registry,
            env,
            control,
        )),
        Scope::DiffText(diff) => Some(review_diff_text_controlled(
            input.root, diff, policy, registry, env, control,
        )),
        _ => None,
    };
    if let Some(review) = changed {
        let review = review.map_err(PipelineError::Diff)?;
        let findings = review.run.findings.clone();
        return Ok(Found {
            run: review.run,
            diff: review.diff,
            semantic: None,
            analyzers: Vec::new(),
            findings,
            skipped: review.skipped,
            deleted: review.deleted,
        });
    }
    let read = match &input.scope {
        Scope::Files(paths) => read_files(input.root, paths),
        _ => read_workspace(input.root),
    };
    let repo_files = repo_files(input.root);
    let run = run_review_controlled(
        policy,
        &read.units,
        Some(&repo_files),
        registry,
        env,
        control,
    )
    .map_err(PipelineError::Finding)?;
    let findings = run.findings.clone();
    Ok(Found {
        run,
        diff: Vec::new(),
        semantic: None,
        analyzers: Vec::new(),
        findings,
        skipped: read.skipped,
        deleted: Vec::new(),
    })
}

fn requested_pack_ids(registry: &CapabilityRegistry, explicit: &[String]) -> Vec<String> {
    let mut ids: Vec<String> = Vec::new();
    for id in explicit
        .iter()
        .cloned()
        .chain(registry.list_packs(None).into_iter().map(|p| p.id.clone()))
    {
        if !ids.contains(&id) {
            ids.push(id);
        }
    }
    ids
}

fn repo_files(root: &Path) -> Vec<RepoFile> {
    list_repo_files(root)
        .into_iter()
        .filter_map(|path| {
            let content = std::fs::read_to_string(root.join(&path)).ok()?;
            Some(RepoFile { path, content })
        })
        .collect()
}

fn read_workspace(root: &Path) -> Read {
    let paths = list_owned_files(root)
        .into_iter()
        .filter(|p| Language::from_path(Path::new(p)).is_some())
        .collect::<Vec<_>>();
    read_paths(root, &paths)
}

fn read_files(root: &Path, paths: &[String]) -> Read {
    let relative: Vec<String> = paths.iter().map(|p| to_repo_relative(root, p)).collect();
    read_paths(root, &relative)
}

fn read_paths(root: &Path, paths: &[String]) -> Read {
    let mut units = Vec::new();
    let mut skips: Vec<(SkipReason, String)> = Vec::new();
    for path in paths {
        match read_text(root, path) {
            Ok(content) if is_generated_content(&content) => {
                skips.push((SkipReason::Generated, path.clone()));
            }
            Ok(content) => units.push(SourceUnit {
                file: path.clone(),
                content,
                changed_lines: None,
            }),
            Err(reason) => skips.push((reason, path.clone())),
        }
    }
    units.sort_by(|a, b| debuggatha_core::js::utf16_cmp(&a.file, &b.file));
    Read {
        units,
        skipped: group(skips),
    }
}

/// A finding's identity must not depend on how its file was named: inside the repository a path is
/// relative with `/` separators; outside it stays as given.
#[must_use]
pub fn to_repo_relative(root: &Path, file: &str) -> String {
    let path = Path::new(file);
    let relative = if path.is_absolute() {
        let real = |p: &Path| p.canonicalize().unwrap_or_else(|_| p.to_path_buf());
        match real(path).strip_prefix(real(root)) {
            Ok(inside) if !inside.as_os_str().is_empty() => inside.to_path_buf(),
            _ => return file.to_owned(),
        }
    } else {
        path.to_path_buf()
    };
    relative
        .components()
        .map(|c| c.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

/// A diff cannot say which findings it did not look at, so it resolves only the files it cites or
/// deletes; a workspace or file review resolves everything inside its scope.
fn sync_scope(root: &Path, scope: &Scope, findings: &[Finding], deleted: &[String]) -> SyncScope {
    match scope {
        Scope::Workspace => SyncScope::Workspace,
        Scope::Files(_) | Scope::Diff { .. } | Scope::DiffText(_) => {
            let mut files: Vec<String> = findings
                .iter()
                .flat_map(debuggatha_core::review_engine::affected_files)
                .chain(deleted.iter().cloned())
                .collect();
            if let Scope::Files(paths) = scope {
                files.extend(paths.iter().map(|p| to_repo_relative(root, p)));
            }
            files.sort();
            files.dedup();
            SyncScope::Files(files)
        }
    }
}

fn changes_of(ledger: &Ledger, sync: &SyncReport, reported: &[Finding]) -> ReviewChanges {
    let entry_of = |finding: &Finding| match default_finding_matcher(finding, &ledger.entries) {
        MatchOutcome::Matched { entry_id, .. } => Some(entry_id),
        MatchOutcome::New => None,
    };
    let reported_ids: Vec<String> = reported.iter().filter_map(entry_of).collect();
    let reference = |id: &String| {
        let entry = ledger.entries.iter().find(|e| &e.id == id)?;
        Some(FindingRef {
            ledger_id: entry.id.clone(),
            title: entry.latest_finding.title.clone(),
            file: entry.fingerprint.file.clone(),
            line: entry
                .latest_finding
                .locations
                .first()
                .and_then(|l| l.lines.map(|r| r.start)),
            rule_id: entry.fingerprint.rule_id.clone(),
            severity: entry.latest_finding.severity,
        })
    };
    let refs = |ids: &[String], only_reported: bool| {
        ids.iter()
            .filter(|id| !only_reported || reported_ids.contains(id))
            .filter_map(reference)
            .collect()
    };
    ReviewChanges {
        introduced: refs(&sync.new_entry_ids, true),
        fixed: refs(&sync.auto_resolved_entry_ids, false),
        reopened: refs(&sync.reopened_entry_ids, true),
    }
}
