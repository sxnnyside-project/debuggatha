//! `.debuggatha/memory.json` (port of `repository-memory/persistence`).

use std::path::{Path, PathBuf};

use super::api::{CURRENT_SCHEMA_VERSION, MemoryStore, create_empty_memory_store};
use crate::env::Env;
use crate::stored::{
    StoreError, deserialize_store, read_if_exists, serialize_store, write_atomically,
};

const WHAT: &str = "repository memory";

/// # Errors
/// The value cannot be written as JSON.
pub fn serialize_memory_store(store: &MemoryStore) -> Result<String, StoreError> {
    serialize_store(store, WHAT)
}

/// # Errors
/// See [`StoreError`]; stricter than the TypeScript reader (`docs/PARITY.md`).
pub fn deserialize_memory_store(json: &str) -> Result<MemoryStore, StoreError> {
    deserialize_store(json, WHAT, CURRENT_SCHEMA_VERSION)
}

#[must_use]
pub fn memory_file_path(repository_root: &Path) -> PathBuf {
    repository_root.join(".debuggatha").join("memory.json")
}

/// A repository with no memory file has an empty store.
///
/// # Errors
/// See [`StoreError`].
pub fn load_memory_store(repository_root: &Path, env: &dyn Env) -> Result<MemoryStore, StoreError> {
    match read_if_exists(&memory_file_path(repository_root), WHAT)? {
        Some(text) => deserialize_memory_store(&text),
        None => Ok(create_empty_memory_store(
            &repository_root.to_string_lossy(),
            env,
        )),
    }
}

/// # Errors
/// I/O failures; the partial file is removed.
pub fn save_memory_store(store: &MemoryStore) -> Result<(), StoreError> {
    let path = memory_file_path(Path::new(&store.repository_root));
    write_atomically(&path, &serialize_memory_store(store)?, WHAT)
}
