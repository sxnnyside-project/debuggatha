//! `review/run`, `review/resume`, `review/cancel` and the events that stop a review.

use super::{
    AnalyzerOptions, AnalyzerParams, Answer, Arc, Cancel, ClientModel, Closing, Control,
    DEFAULT_PAGE, Daemon, Emitter, Failure, Finding, Instant, Mode, Mutex, Ordering, Outcome,
    PROGRESS_EVERY, PROTOCOL, PoisonError, Request, ReviewDepth, ReviewInput, ReviewOutput,
    ReviewParams, Scope, ScopeParams, SemanticInput, SemanticOptions, SemanticProvider, Session,
    Suspending, SystemEnv, Value, execute_review_with, fail, inside_root, is_ignored_path, json,
    lock, lock_running, lock_suspended, params, pipeline_failure, repository_stamp,
};

impl Daemon {
    pub(super) fn analyzer_options(&self, requested: &AnalyzerParams) -> AnalyzerOptions {
        AnalyzerOptions {
            mode: if requested.mode.as_deref() == Some("off") {
                Mode::Off
            } else {
                Mode::Auto
            },
            // Enabling is the daemon's, never the request's.
            enable: self.config.enabled_analyzers.clone(),
            disable: requested.disable.clone(),
            timeout_seconds: requested.timeout_seconds.filter(|s| *s > 0),
        }
    }

    pub(super) fn scope(&self, scope: &ScopeParams) -> Result<Scope, Failure> {
        match scope.kind.as_str() {
            "workspace" => Ok(Scope::Workspace),
            "files" => {
                let paths = scope
                    .paths
                    .iter()
                    .map(|p| inside_root(&self.config.root, p))
                    .collect::<Result<Vec<_>, _>>()?;
                let kept: Vec<String> = paths
                    .iter()
                    .filter(|p| !scope.owned || !is_ignored_path(Some(&self.config.root), p))
                    .cloned()
                    .collect();
                if kept.is_empty() && !paths.is_empty() {
                    return Err(fail(
                        "nothing_to_review",
                        "None of the files is part of the project's own code.",
                    ));
                }
                Ok(Scope::Files(kept))
            }
            "diff" => Ok(match &scope.diff {
                Some(text) => Scope::DiffText(text.clone()),
                None => Scope::Diff {
                    base: scope.base.clone(),
                },
            }),
            other => Err(fail(
                "invalid_request",
                format!("The scope \"{other}\" is not workspace, files or diff."),
            )),
        }
    }

    pub(super) fn review_run(&self, request: &Request, emit: &Emitter) -> Answer {
        let cancel = Cancel::default();
        // A client that cannot be asked gets the second form: the review may stop and wait.
        // A stateless client says with each call (`inputRequired`) what it can answer.
        let suspends = request.params["semantic"] == json!(true)
            && self.config.client_model
            && (request.params["inputRequired"] == json!(true)
                || (!self.sampling.load(Ordering::SeqCst)
                    && self.input_required.load(Ordering::SeqCst)));
        let session = suspends.then(|| {
            let token = format!("r-{:x}", self.tokens.fetch_add(1, Ordering::Relaxed) + 1);
            let stamp = repository_stamp(&self.config.root);
            let session = Session::new(token.clone(), stamp, &request.id, cancel.clone());
            lock_suspended(&self.suspended).insert(token, Arc::clone(&session));
            session
        });
        let outcome = self.run_review(request, emit, cancel, session.as_deref());
        let Some(session) = session else {
            return Answer::Now(outcome);
        };
        lock_suspended(&self.suspended).retain(|_, s| !Arc::ptr_eq(s, &session));
        match session.closing() {
            Closing::Original => Answer::Now(outcome),
            Closing::To(id) => {
                emit(self.render(&id, outcome));
                Answer::Sent
            }
            Closing::Nobody => Answer::Sent,
        }
    }

    pub(super) fn review_resume(&self, request: &Request) -> Answer {
        let outcome = (|| {
            let token = request.params["token"]
                .as_str()
                .ok_or_else(|| fail("invalid_request", "The parameter \"token\" is required."))?;
            let responses = request.params["responses"].as_object().ok_or_else(|| {
                fail(
                    "invalid_request",
                    "The parameter \"responses\" is required.",
                )
            })?;
            let session = lock_suspended(&self.suspended)
                .get(token)
                .cloned()
                .ok_or_else(|| fail("not_found", "No review is waiting under that token."))?;
            if repository_stamp(&self.config.root) != session.stamp {
                session.cancel.cancel();
                return Err(fail(
                    "conflict",
                    "The repository changed while the review waited; run it again.",
                ));
            }
            if session.deliver(&request.id, Value::Object(responses.clone())) {
                Ok(())
            } else {
                Err(fail("conflict", "That review is not waiting for answers."))
            }
        })();
        match outcome {
            Ok(()) => Answer::Sent,
            Err(failure) => Answer::Now(Err(failure)),
        }
    }

    pub(super) fn run_review(
        &self,
        request: &Request,
        emit: &Emitter,
        cancel: Cancel,
        session: Option<&Session>,
    ) -> Outcome {
        let asked: ReviewParams = params(request)?;
        let scope = self.scope(&asked.scope)?;
        let persist = asked.persist.unwrap_or(true);
        let started = Instant::now();
        let sampled = (asked.semantic
            && self.config.client_model
            && self.sampling.load(Ordering::SeqCst)
            && session.is_none())
        .then(|| ClientModel::new(&self.asked, emit, cancel.clone()));
        let parked = session.map(|s| Suspending::new(s, emit, cancel.clone()));
        let local = (asked.semantic && sampled.is_none() && parked.is_none())
            .then_some(())
            .and(self.config.semantic.as_ref());
        let provider: Option<&dyn SemanticProvider> = sampled
            .as_ref()
            .map(|p| p as &dyn SemanticProvider)
            .or(parked.as_ref().map(|p| p as &dyn SemanticProvider))
            .or(local.map(|p| p as &dyn SemanticProvider));
        let input = ReviewInput {
            root: &self.config.root,
            scope,
            depth: ReviewDepth::Full,
            pack_ids: asked.pack_ids,
            policy_id: asked.policy_id,
            source_name: "debuggatha-daemon",
            persist,
            include_baselined: asked.include_baselined,
            analyzers: Some(self.analyzer_options(&asked.analyzers)),
            analyzer_search: self.config.analyzer_search.clone(),
            semantic: provider.map(|provider| SemanticInput {
                provider,
                options: SemanticOptions::default(),
            }),
        };
        lock_running(&self.running).insert(request.id.clone(), cancel.clone());
        let control = self.control(&request.id, cancel, emit);
        let turn = persist.then(|| lock(&self.writes));
        let output = execute_review_with(&input, &SystemEnv, &control);
        drop(turn);
        lock_running(&self.running).remove(&request.id);
        let output = output.map_err(|e| pipeline_failure(&e))?;
        Ok(review_result(
            &output,
            asked.limit.unwrap_or(DEFAULT_PAGE),
            started,
        ))
    }

    /// What lets a review be stopped, and, when the client wants it, hear its progress.
    pub(super) fn control(&self, request: &str, cancel: Cancel, emit: &Emitter) -> Control {
        let control = Control::new(cancel);
        if !self.progress.load(Ordering::SeqCst) {
            return control;
        }
        let (emit, request) = (Arc::clone(emit), request.to_owned());
        let last = Mutex::new((None::<&'static str>, Instant::now()));
        control.listening(move |stage, done, total, message| {
            let mut last = last.lock().unwrap_or_else(PoisonError::into_inner);
            let changed = last.0 != Some(stage.name());
            if !changed && last.1.elapsed() < PROGRESS_EVERY {
                return;
            }
            *last = (Some(stage.name()), Instant::now());
            emit(json!({
                "v": PROTOCOL, "type": "event", "method": "progress",
                "params": { "request": request, "stage": stage.name(), "done": done,
                            "total": total, "message": message },
            }));
        })
    }

    pub(super) fn review_cancel(&self, request: &Request) -> Outcome {
        let target = request.params["target"]
            .as_str()
            .ok_or_else(|| fail("invalid_request", "The parameter \"target\" is required."))?;
        self.cancel(target)
            .then(|| json!({ "target": target }))
            .ok_or_else(|| fail("not_found", "No review is running under that request id."))
    }

    pub(super) fn cancel(&self, target: &str) -> bool {
        lock_running(&self.running).get(target).is_some_and(|c| {
            c.cancel();
            true
        })
    }

    /// The session is over: stop every review still running.
    pub fn cancel_all(&self) {
        for cancel in lock_running(&self.running).values() {
            cancel.cancel();
        }
        self.asked.drop_all();
    }

    /// An event or a response from the client. The only event is `cancel`; any other is ignored,
    /// as the tolerant reader of the protocol requires. A response answers a request of the
    /// daemon's (`model/complete`). `true` when the line was one of them, not a request.
    pub fn event(&self, line: &str) -> bool {
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            return false;
        };
        if value["type"] == "response" {
            self.asked.answer(&value);
            return true;
        }
        if value["type"] != "event" {
            return false;
        }
        if let (true, Some(target)) = (
            value["method"] == "cancel",
            value["params"]["target"].as_str(),
        ) {
            self.cancel(target);
        }
        true
    }
}

/// `review-job-result`: counts and records, plus the first page of findings by severity, and what
/// the review could not check. An empty page is never "clean" unless `coverage.complete` says so.
fn review_result(output: &ReviewOutput, limit: usize, started: Instant) -> Value {
    let mut findings: Vec<&Finding> = output.result.findings.iter().collect();
    findings.sort_by_key(|f| f.severity);
    let truncated = findings.len() > limit;
    findings.truncate(limit);
    json!({
        "findings": output.result.findings.len(),
        "analyzers": output.analyzers,
        "semantic": output.semantic,
        "changes": {
            "introduced": output.changes.introduced.len(),
            "fixed": output.changes.fixed.len(),
            "reopened": output.changes.reopened.len(),
        },
        "durationMs": u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
        "coverage": output.coverage,
        "findingsPage": { "findings": findings, "truncated": truncated },
        "policyId": output.policy.id,
        "baselined": output.baselined,
    })
}
