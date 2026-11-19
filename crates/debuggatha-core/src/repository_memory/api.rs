use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::item::{MemoryItem, MemoryOrigin, MemoryTransitionError, transition_memory};
use super::lifecycle::MemoryStatus;
use crate::env::Env;

pub const CURRENT_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct MemoryStore {
    pub schema_version: u32,
    pub repository_root: String,
    pub items: Vec<MemoryItem>,
    pub created_at: String,
    pub updated_at: String,
}

#[must_use]
pub fn create_empty_memory_store(repository_root: &str, env: &dyn Env) -> MemoryStore {
    let now = env.now();
    MemoryStore {
        schema_version: CURRENT_SCHEMA_VERSION,
        repository_root: repository_root.to_owned(),
        items: Vec::new(),
        created_at: now.clone(),
        updated_at: now,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MemoryError {
    NotFound(String),
    Transition(MemoryTransitionError),
}

impl fmt::Display for MemoryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound(id) => write!(f, "No memory item with id \"{id}\" in this store."),
            Self::Transition(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for MemoryError {}

fn with_items(store: &MemoryStore, items: Vec<MemoryItem>, env: &dyn Env) -> MemoryStore {
    let mut next = store.clone();
    next.items = items;
    next.updated_at = env.now();
    next
}

fn find<'a>(store: &'a MemoryStore, id: &str) -> Result<&'a MemoryItem, MemoryError> {
    store
        .items
        .iter()
        .find(|i| i.id == id)
        .ok_or_else(|| MemoryError::NotFound(id.to_owned()))
}

fn move_item(
    store: &MemoryStore,
    id: &str,
    to: MemoryStatus,
    origin: MemoryOrigin,
    comment: Option<String>,
    env: &dyn Env,
) -> Result<MemoryStore, MemoryError> {
    let updated = transition_memory(find(store, id)?, to, origin, comment, env)
        .map_err(MemoryError::Transition)?;
    let items = store
        .items
        .iter()
        .map(|i| {
            if i.id == updated.id {
                updated.clone()
            } else {
                i.clone()
            }
        })
        .collect();
    Ok(with_items(store, items, env))
}

fn user(actor: &str) -> MemoryOrigin {
    MemoryOrigin::User {
        actor: actor.to_owned(),
    }
}

#[must_use]
pub fn add_memory_item(store: &MemoryStore, item: MemoryItem, env: &dyn Env) -> MemoryStore {
    let mut items = store.items.clone();
    items.push(item);
    with_items(store, items, env)
}

/// Detection proposes; nothing here confirms or activates.
///
/// # Errors
/// `NotFound`, or `Transition` when the lifecycle forbids the move. The same for every function below.
pub fn suggest_memory(
    store: &MemoryStore,
    id: &str,
    session_id: &str,
    env: &dyn Env,
) -> Result<MemoryStore, MemoryError> {
    let origin = MemoryOrigin::Detection {
        session_id: session_id.to_owned(),
    };
    move_item(store, id, MemoryStatus::Suggested, origin, None, env)
}

/// # Errors
/// See [`suggest_memory`].
pub fn confirm_memory(
    store: &MemoryStore,
    id: &str,
    actor: &str,
    comment: Option<String>,
    env: &dyn Env,
) -> Result<MemoryStore, MemoryError> {
    move_item(
        store,
        id,
        MemoryStatus::Confirmed,
        user(actor),
        comment,
        env,
    )
}

/// # Errors
/// See [`suggest_memory`].
pub fn activate_memory(
    store: &MemoryStore,
    id: &str,
    actor: &str,
    env: &dyn Env,
) -> Result<MemoryStore, MemoryError> {
    move_item(store, id, MemoryStatus::Active, user(actor), None, env)
}

/// # Errors
/// See [`suggest_memory`].
pub fn reject_memory(
    store: &MemoryStore,
    id: &str,
    actor: &str,
    comment: Option<String>,
    env: &dyn Env,
) -> Result<MemoryStore, MemoryError> {
    move_item(store, id, MemoryStatus::Archived, user(actor), comment, env)
}

/// # Errors
/// See [`suggest_memory`].
pub fn deprecate_memory(
    store: &MemoryStore,
    id: &str,
    actor: &str,
    comment: Option<String>,
    env: &dyn Env,
) -> Result<MemoryStore, MemoryError> {
    move_item(
        store,
        id,
        MemoryStatus::Deprecated,
        user(actor),
        comment,
        env,
    )
}

/// # Errors
/// See [`suggest_memory`].
pub fn reactivate_memory(
    store: &MemoryStore,
    id: &str,
    actor: &str,
    env: &dyn Env,
) -> Result<MemoryStore, MemoryError> {
    move_item(store, id, MemoryStatus::Active, user(actor), None, env)
}

/// # Errors
/// See [`suggest_memory`].
pub fn archive_memory(
    store: &MemoryStore,
    id: &str,
    actor: &str,
    comment: Option<String>,
    env: &dyn Env,
) -> Result<MemoryStore, MemoryError> {
    move_item(store, id, MemoryStatus::Archived, user(actor), comment, env)
}

/// # Errors
/// `NotFound` for an unknown id.
pub fn remove_memory_item(
    store: &MemoryStore,
    id: &str,
    env: &dyn Env,
) -> Result<MemoryStore, MemoryError> {
    find(store, id)?;
    let items = store.items.iter().filter(|i| i.id != id).cloned().collect();
    Ok(with_items(store, items, env))
}
