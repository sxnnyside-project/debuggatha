//! The `findings/*` methods: the ledger as a client sees it.

use super::{
    DEFAULT_PAGE, Daemon, LedgerEntryFilter, LedgerError, LifecycleStatus, ListParams,
    MemoryConfidence, MemoryDetail, MemoryEvidence, MemoryOrigin, NewMemoryItem, Outcome, Request,
    SystemEnv, TransitionOrigin, Value, activate_memory, add_memory_item, can_transition_finding,
    confirm_memory, create_memory_item, fail, finding_id, get_entry, internal, json, list_entries,
    load_ledger, load_memory_store, lock, params, save_ledger, save_memory_store, store_failure,
    suggest_memory, unknown_finding, update_finding_status,
};

impl Daemon {
    pub(super) fn findings_list(&self, request: &Request) -> Outcome {
        let asked: ListParams = params(request)?;
        let ledger = load_ledger(&self.config.root, &SystemEnv).map_err(|e| store_failure(&e))?;
        let filter = LedgerEntryFilter {
            status: (!asked.status.is_empty()).then_some(asked.status),
            severity: (!asked.severity.is_empty()).then_some(asked.severity),
            category: (!asked.category.is_empty()).then_some(asked.category),
            file: asked.file,
        };
        let entries = list_entries(&ledger, &filter);
        let start = match &asked.cursor {
            Some(cursor) => cursor
                .parse::<usize>()
                .map_err(|_| fail("invalid_request", "The cursor is not one this daemon gave."))?,
            None => 0,
        };
        let limit = asked.limit.unwrap_or(DEFAULT_PAGE).max(1);
        let page: Vec<_> = entries.iter().skip(start).take(limit).collect();
        let next = (start + page.len() < entries.len()).then(|| (start + page.len()).to_string());
        Ok(json!({ "entries": page, "nextCursor": next, "total": entries.len() }))
    }

    pub(super) fn findings_get(&self, request: &Request) -> Outcome {
        let id = finding_id(request)?;
        let ledger = load_ledger(&self.config.root, &SystemEnv).map_err(|e| store_failure(&e))?;
        get_entry(&ledger, &id)
            .map(|entry| json!({ "entry": entry }))
            .ok_or_else(unknown_finding)
    }

    /// Every lifecycle event of every finding, newest first.
    pub(super) fn findings_history(&self) -> Outcome {
        let ledger = load_ledger(&self.config.root, &SystemEnv).map_err(|e| store_failure(&e))?;
        let mut events: Vec<Value> = ledger
            .entries
            .iter()
            .flat_map(|entry| {
                entry.history.iter().filter_map(|event| {
                    let mut event = serde_json::to_value(event).ok()?;
                    event["findingId"] = json!(entry.id);
                    event["fingerprint"] = serde_json::to_value(&entry.fingerprint).ok()?;
                    Some(event)
                })
            })
            .collect();
        events.sort_by(|a, b| b["timestamp"].as_str().cmp(&a["timestamp"].as_str()));
        Ok(json!({ "total": events.len(), "events": events }))
    }

    pub(super) fn findings_update(&self, request: &Request) -> Outcome {
        let id = finding_id(request)?;
        let status: LifecycleStatus = serde_json::from_value(request.params["status"].clone())
            .map_err(|_| {
                fail(
                    "invalid_request",
                    "The parameter \"status\" is not a lifecycle status.",
                )
            })?;
        let comment = request.params["comment"].as_str().map(str::to_owned);
        let _turn = lock(&self.writes);
        let ledger = load_ledger(&self.config.root, &SystemEnv).map_err(|e| store_failure(&e))?;
        let updated = update_finding_status(
            &ledger,
            &id,
            status,
            TransitionOrigin::Manual {
                actor: "daemon".to_owned(),
            },
            comment,
            &SystemEnv,
        )
        .map_err(|e| match e {
            LedgerError::EntryNotFound(_) => unknown_finding(),
            illegal @ LedgerError::IllegalTransition { .. } => {
                fail("conflict", illegal.to_string())
            }
        })?;
        save_ledger(&updated).map_err(|e| store_failure(&e))?;
        Ok(json!({ "entry": get_entry(&updated, &id) }))
    }

    /// Everything needed to act on one finding: where it is, why it is a problem, how to fix it,
    /// and what to do if the code is right and the rule is wrong.
    pub(super) fn findings_explain(&self, request: &Request) -> Outcome {
        let id = finding_id(request)?;
        let ledger = load_ledger(&self.config.root, &SystemEnv).map_err(|e| store_failure(&e))?;
        let entry = get_entry(&ledger, &id).ok_or_else(unknown_finding)?;
        let entry = serde_json::to_value(entry).map_err(|e| internal(&e))?;
        let finding = &entry["latestFinding"];
        let file = entry["fingerprint"]["file"].as_str().unwrap_or_default();
        let rule = entry["fingerprint"]["ruleId"].as_str();
        let guidance = rule.and_then(crate::detect::guidance_for);
        let recommendation = &finding["recommendations"][0];
        let excerpt = finding["evidence"]
            .as_array()
            .and_then(|all| all.iter().find(|e| e["kind"] == "code"))
            .map(|e| e["excerpt"].clone());
        let history: Vec<Value> = entry["history"]
            .as_array()
            .map(|events| {
                events
                    .iter()
                    .map(|e| json!({ "timestamp": e["timestamp"], "state": e["newState"], "comment": e["comment"] }))
                    .collect()
            })
            .unwrap_or_default();
        Ok(json!({
            "finding": {
                "ledgerId": entry["id"], "status": entry["status"],
                "severity": finding["severity"], "confidence": finding["confidence"],
                "category": finding["category"], "title": finding["title"], "file": file,
                "line": finding["locations"][0]["lines"]["start"],
                "column": finding["locations"][0]["columns"]["start"],
                "excerpt": excerpt,
            },
            "rule": { "id": rule, "pack": finding["originatingPack"]["id"] },
            "why": guidance.map_or_else(|| finding["explanation"].clone(), |g| json!(g.why)),
            "fix": recommendation["summary"].as_str()
                .unwrap_or("No specific fix is recorded for this rule."),
            "example": recommendation["example"],
            "edit": recommendation["edit"],
            "ifYouAcceptIt": {
                "inlineComment": crate::syntax::inline_suppression_comment(
                    file, rule.unwrap_or("<rule>"), "<why this is acceptable>"),
                "durable": format!(
                    "findings/suppress with this id and a reason accepts every {} finding in {file}.",
                    rule.unwrap_or("such")),
            },
            "history": history,
        }))
    }

    /// Accepts a finding with a reason: every finding of its rule in its file stops being
    /// reported, the reason goes to the repository's memory, and the ledger entry is dismissed.
    pub(super) fn findings_suppress(&self, request: &Request) -> Outcome {
        let id = finding_id(request)?;
        let reason = request.params["reason"]
            .as_str()
            .map(str::trim)
            .filter(|r| r.chars().count() >= 8)
            .ok_or_else(|| {
                fail(
                    "invalid_request",
                    "A reason of at least 8 characters is required.",
                )
            })?;
        let actor = "daemon";
        let env = &SystemEnv;
        let _turn = lock(&self.writes);
        let ledger = load_ledger(&self.config.root, env).map_err(|e| store_failure(&e))?;
        let entry = get_entry(&ledger, &id).ok_or_else(unknown_finding)?;
        let Some(rule) = entry.fingerprint.rule_id.clone() else {
            return Err(fail(
                "conflict",
                "This finding has no rule behind it, so there is nothing to accept by rule.",
            ));
        };
        let file = entry.fingerprint.file.clone();
        let item = create_memory_item(
            NewMemoryItem {
                confidence: MemoryConfidence::Documented,
                rationale: reason.to_owned(),
                evidence: vec![MemoryEvidence {
                    file: file.clone(),
                    detail: format!("Accepted: {}", entry.latest_finding.title),
                }],
                detail: MemoryDetail::Suppression {
                    fingerprint_file: file.clone(),
                    fingerprint_rule_id: Some(rule.clone()),
                    fingerprint_category: entry.fingerprint.category.clone(),
                },
                origin: MemoryOrigin::User {
                    actor: actor.to_owned(),
                },
            },
            env,
        );
        let item_id = item.id.clone();
        let session = format!("daemon-{item_id}");
        let store = load_memory_store(&self.config.root, env).map_err(|e| store_failure(&e))?;
        let store = add_memory_item(&store, item, env);
        let store = suggest_memory(&store, &item_id, &session, env)
            .and_then(|s| confirm_memory(&s, &item_id, actor, Some(reason.to_owned()), env))
            .and_then(|s| activate_memory(&s, &item_id, actor, env))
            .map_err(|e| internal(&e))?;
        save_memory_store(&store).map_err(|e| store_failure(&e))?;
        if can_transition_finding(entry.status, LifecycleStatus::Dismissed) {
            let updated = update_finding_status(
                &ledger,
                &id,
                LifecycleStatus::Dismissed,
                TransitionOrigin::Manual {
                    actor: actor.to_owned(),
                },
                Some(reason.to_owned()),
                env,
            )
            .map_err(|e| internal(&e))?;
            save_ledger(&updated).map_err(|e| store_failure(&e))?;
        }
        Ok(json!({
            "memoryItemId": item_id, "file": file, "ruleId": rule, "reason": reason,
            "effect": format!("Every {rule} finding in {file} stops being reported."),
        }))
    }
}
