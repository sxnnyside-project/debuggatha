//! `memory/list` and `memory/transition`: Repository Memory.

use super::{
    Daemon, MemoryError, Outcome, Request, SystemEnv, activate_memory, archive_memory,
    confirm_memory, deprecate_memory, fail, finding_id, json, load_memory_store, lock,
    reject_memory, remove_memory_item, save_memory_store, store_failure, suggest_memory,
};

impl Daemon {
    pub(super) fn memory_list(&self, request: &Request) -> Outcome {
        let status = request.params["status"].as_str();
        let store =
            load_memory_store(&self.config.root, &SystemEnv).map_err(|e| store_failure(&e))?;
        let items: Vec<_> = store
            .items
            .iter()
            .filter(|i| status.is_none_or(|s| i.status.as_str() == s))
            .collect();
        Ok(json!({ "items": items }))
    }

    pub(super) fn memory_transition(&self, request: &Request) -> Outcome {
        let id = finding_id(request)?;
        let action = request.params["action"].as_str().unwrap_or_default();
        let comment = request.params["comment"].as_str().map(str::to_owned);
        let (env, actor) = (&SystemEnv, "daemon");
        let _turn = lock(&self.writes);
        let store = load_memory_store(&self.config.root, env).map_err(|e| store_failure(&e))?;
        let moved = match action {
            "suggest" => suggest_memory(&store, &id, actor, env),
            "confirm" => confirm_memory(&store, &id, actor, comment, env),
            "activate" => activate_memory(&store, &id, actor, env),
            "reject" => reject_memory(&store, &id, actor, comment, env),
            "deprecate" => deprecate_memory(&store, &id, actor, comment, env),
            "archive" => archive_memory(&store, &id, actor, comment, env),
            "remove" => remove_memory_item(&store, &id, env),
            _ => {
                return Err(fail(
                    "invalid_request",
                    "The action is not one of suggest, confirm, activate, reject, deprecate, archive or remove.",
                ));
            }
        }
        .map_err(|e| match e {
            MemoryError::NotFound(_) => fail("not_found", "No memory item has that id."),
            MemoryError::Transition(t) => fail("conflict", t.to_string()),
        })?;
        save_memory_store(&moved).map_err(|e| store_failure(&e))?;
        Ok(json!({ "item": moved.items.iter().find(|i| i.id == id) }))
    }
}
