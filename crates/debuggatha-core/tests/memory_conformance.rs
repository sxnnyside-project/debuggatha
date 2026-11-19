//! Differential tests against the TypeScript repository memory (ADR-0006, decision 8).
//!
//! `tests/fixtures/memory-conformance.json` is written by `scripts/parity/gen-memory-fixtures.ts` at tag `ts-engine-final`.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)] // a failed expect is a failed test

mod common;

use common::{CountingEnv, fixture};
use debuggatha_core::repository_memory::{
    MemoryConfidence, MemoryDetail, MemoryEvidence, MemoryItem, MemoryOrigin, MemoryStatus,
    MemoryStore, NewMemoryItem, activate_memory, add_memory_item, archive_memory, confirm_memory,
    create_empty_memory_store, create_memory_item, deprecate_memory, deserialize_memory_store,
    filter_suppressed_findings, load_memory_store, reactivate_memory, reject_memory,
    remove_memory_item, save_memory_store, serialize_memory_store, suggest_memory,
};
use debuggatha_core::review_engine::Finding;
use serde_json::{Value, json};

fn project(item: &MemoryItem) -> Value {
    let mut value = serde_json::to_value(item).unwrap();
    let object = value.as_object_mut().unwrap();
    object.remove("createdAt");
    object.remove("updatedAt");
    for event in object["history"].as_array_mut().unwrap() {
        event.as_object_mut().unwrap().remove("timestamp");
    }
    value
}

fn new_item(input: &Value) -> NewMemoryItem {
    let mut detail = input.clone();
    let object = detail.as_object_mut().unwrap();
    for base in ["confidence", "rationale", "evidence", "origin"] {
        object.remove(base);
    }
    NewMemoryItem {
        confidence: serde_json::from_value::<MemoryConfidence>(input["confidence"].clone())
            .unwrap(),
        rationale: input["rationale"].as_str().unwrap().to_owned(),
        evidence: serde_json::from_value::<Vec<MemoryEvidence>>(input["evidence"].clone()).unwrap(),
        detail: serde_json::from_value::<MemoryDetail>(detail).unwrap(),
        origin: serde_json::from_value::<MemoryOrigin>(input["origin"].clone()).unwrap(),
    }
}

fn run_call(
    store: &MemoryStore,
    action: &Value,
    env: &CountingEnv,
) -> Result<MemoryStore, debuggatha_core::repository_memory::MemoryError> {
    let id = action["itemId"].as_str().unwrap();
    let comment = action["comment"].as_str().map(str::to_owned);
    match action["fn"].as_str().unwrap() {
        "suggest" => suggest_memory(store, id, "session-1", env),
        "confirm" => confirm_memory(store, id, "a", comment, env),
        "activate" => activate_memory(store, id, "a", env),
        "reject" => reject_memory(store, id, "a", comment, env),
        "deprecate" => deprecate_memory(store, id, "a", comment, env),
        "reactivate" => reactivate_memory(store, id, "a", env),
        "archive" => archive_memory(store, id, "a", comment, env),
        "remove" => remove_memory_item(store, id, env),
        other => panic!("unknown fn {other}"),
    }
}

/// Replays the scenario; returns the final store.
fn replay(doc: &Value, env: &CountingEnv) -> MemoryStore {
    let mut store = create_empty_memory_store("/repo", env);
    for (i, step) in doc["steps"].as_array().unwrap().iter().enumerate() {
        let action = &step["action"];
        let expected = &step["expected"];
        let at = format!("step {} ({})", i + 1, action["type"]);
        match action["type"].as_str().unwrap() {
            "create" => {
                let item = create_memory_item(new_item(&action["input"]), env);
                store = add_memory_item(&store, item, env);
            }
            "call" => match run_call(&store, action, env) {
                Ok(next) => {
                    assert!(
                        expected.get("error").is_none(),
                        "{at}: TypeScript refused, Rust accepted"
                    );
                    store = next;
                }
                Err(error) => {
                    assert_eq!(json!(error.to_string()), expected["error"], "{at}");
                    continue;
                }
            },
            "filter" => {
                let findings: Vec<Finding> =
                    serde_json::from_value(action["findings"].clone()).unwrap();
                let out = filter_suppressed_findings(&findings, &store);
                let kept: Vec<&str> = out.findings.iter().map(|f| f.id.as_str()).collect();
                assert_eq!(json!(kept), expected["kept"], "{at}: kept");
                let suppressed: Vec<Value> = out
                    .suppressed
                    .iter()
                    .map(|s| json!({ "findingId": s.finding.id, "memoryItemId": s.memory_item_id, "reason": s.reason }))
                    .collect();
                assert_eq!(
                    json!(suppressed),
                    expected["suppressed"],
                    "{at}: suppressed"
                );
                continue;
            }
            other => panic!("unknown action {other}"),
        }
        let items: Vec<Value> = store.items.iter().map(project).collect();
        assert_eq!(json!(items), expected["items"], "{at}: items");
    }
    store
}

#[test]
fn the_whole_lifecycle_matches_the_typescript_memory() {
    replay(&fixture("memory-conformance.json"), &CountingEnv::new("m"));
}

#[test]
fn serialized_store_is_byte_identical_and_reads_back() {
    let doc = fixture("memory-conformance.json");
    let store = replay(&doc, &CountingEnv::new("m"));
    let text = serialize_memory_store(&store).unwrap();
    assert_eq!(text, doc["serialized"].as_str().unwrap());
    assert_eq!(deserialize_memory_store(&text).unwrap(), store);
}

#[test]
fn only_active_items_filter_findings() {
    // The invariant in CLAUDE.md, stated directly rather than through the scenario.
    let doc = fixture("memory-conformance.json");
    let store = replay(&doc, &CountingEnv::new("m"));
    let findings: Vec<Finding> = serde_json::from_value(
        doc["steps"]
            .as_array()
            .unwrap()
            .iter()
            .find(|s| s["action"]["type"] == "filter")
            .unwrap()["action"]["findings"]
            .clone(),
    )
    .unwrap();
    let result = filter_suppressed_findings(&findings, &store);
    assert!(
        !result.suppressed.is_empty(),
        "the scenario must exercise the filter"
    );
    let covering: Vec<&str> = result
        .suppressed
        .iter()
        .map(|s| s.memory_item_id.as_str())
        .collect();
    for id in covering {
        let item = store.items.iter().find(|i| i.id == id).unwrap();
        assert_eq!(
            item.status,
            MemoryStatus::Active,
            "{id} filtered while {:?}",
            item.status
        );
    }
}

#[test]
fn a_real_six_kind_store_reads_and_rewrites_identically() {
    let value = fixture("memory-real.json");
    let store = deserialize_memory_store(&value.to_string()).unwrap();
    assert_eq!(store.items.len(), 6);
    assert_eq!(serde_json::to_value(&store).unwrap(), value);
}

#[test]
fn the_reader_refuses_what_the_typescript_reader_refuses() {
    for case in fixture("memory-conformance.json")["refusals"]
        .as_array()
        .unwrap()
    {
        let error = deserialize_memory_store(case["json"].as_str().unwrap()).unwrap_err();
        let message = error.to_string();
        assert!(
            message.contains(case["message"].as_str().unwrap()),
            "{message}"
        );
        assert!(
            message.starts_with("Cannot read repository memory"),
            "{message}"
        );
    }
}

#[test]
fn save_is_atomic_and_load_round_trips() {
    let env = CountingEnv::new("m");
    let dir = std::env::temp_dir().join(format!("debuggatha-memory-{}", std::process::id()));
    drop(std::fs::remove_dir_all(&dir));
    std::fs::create_dir_all(&dir).unwrap();
    assert_eq!(load_memory_store(&dir, &env).unwrap().items, []);
    let store = create_empty_memory_store(&dir.to_string_lossy(), &env);
    save_memory_store(&store).unwrap();
    assert_eq!(load_memory_store(&dir, &env).unwrap(), store);
    let names: Vec<_> = std::fs::read_dir(dir.join(".debuggatha"))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(names, ["memory.json"]);
    std::fs::remove_dir_all(&dir).unwrap();
}
