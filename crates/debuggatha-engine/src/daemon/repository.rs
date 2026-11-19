//! `baseline/create`, `repository/*` and `analyzers/list`.

use super::{
    ADAPTERS, AnalyzerOptions, Daemon, Outcome, PipelineError, Request, ReviewDepth, ReviewInput,
    ReviewParams, Scope, SystemEnv, Value, assemble_policy, build_repository_context,
    create_baseline, fail, inventory, json, lock, pipeline_failure, summarize_capabilities,
};

impl Daemon {
    pub(super) fn baseline_create(&self, request: &Request) -> Outcome {
        let asked: ReviewParams = serde_json::from_value(json!({
            "scope": { "kind": "workspace" },
            "packIds": request.params.get("packIds").cloned().unwrap_or_else(|| json!([])),
            "analyzers": request.params.get("analyzers").cloned().unwrap_or_else(|| json!({})),
        }))
        .map_err(|e| {
            fail(
                "invalid_request",
                format!("The parameters are not valid: {e}."),
            )
        })?;
        let _turn = lock(&self.writes);
        let (baseline, runs) = create_baseline(
            &ReviewInput {
                root: &self.config.root,
                scope: Scope::Workspace,
                depth: ReviewDepth::Full,
                pack_ids: asked.pack_ids,
                policy_id: None,
                source_name: "debuggatha-daemon",
                persist: false,
                include_baselined: true,
                analyzers: Some(self.analyzer_options(&asked.analyzers)),
                analyzer_search: self.config.analyzer_search.clone(),
                semantic: None,
            },
            &SystemEnv,
        )
        .map_err(|e| pipeline_failure(&e))?;
        let findings: u32 = baseline.entries.iter().map(|e| e.count).sum();
        Ok(json!({ "findings": findings, "analyzers": runs }))
    }

    pub(super) fn repository_context(&self) -> Outcome {
        let context = build_repository_context(&self.config.root, &SystemEnv, None)
            .map_err(|e| pipeline_failure(&PipelineError::Scan(e)))?;
        Ok(json!({ "context": context }))
    }

    /// The Review Policy a review of this repository would assemble, with the packs it names.
    pub(super) fn repository_policy(&self, request: &Request) -> Outcome {
        let packs: Vec<String> = request.params["packIds"]
            .as_array()
            .map(|all| {
                all.iter()
                    .filter_map(|p| p.as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default();
        let (_, policy, requested) = assemble_policy(&self.config.root, &packs, None, &SystemEnv)
            .map_err(|e| pipeline_failure(&e))?;
        Ok(json!({ "policy": policy, "requestedPackIds": requested }))
    }

    pub(super) fn repository_summary(&self) -> Outcome {
        let context = build_repository_context(&self.config.root, &SystemEnv, None)
            .map_err(|e| pipeline_failure(&PipelineError::Scan(e)))?;
        Ok(json!({ "summary": summarize_capabilities(&context.capabilities) }))
    }

    pub(super) fn analyzers_list(&self) -> Value {
        let options = AnalyzerOptions {
            enable: self.config.enabled_analyzers.clone(),
            ..AnalyzerOptions::default()
        };
        let entries = inventory(
            &self.config.root,
            &options,
            &self.config.analyzer_search,
            ADAPTERS,
        );
        let list: Vec<Value> = entries
            .iter()
            .map(|e| {
                json!({
                    "id": e.id, "name": e.name, "license": e.license, "trust": e.trust,
                    "applies": e.applies, "installed": e.installed, "version": e.version,
                    "willRun": e.will_run, "reason": e.reason,
                })
            })
            .collect();
        json!({ "analyzers": list })
    }
}
