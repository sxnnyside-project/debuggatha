//! `.debuggatha/ledger.json` (port of `findings-ledger/persistence`).

use std::path::{Path, PathBuf};

use super::entry::{CURRENT_SCHEMA_VERSION, Ledger, create_empty_ledger};
use crate::env::Env;
use crate::stored::{
    StoreError, deserialize_store, read_if_exists, serialize_store, write_atomically,
};

const WHAT: &str = "ledger";

/// # Errors
/// The value cannot be written as JSON.
pub fn serialize_ledger(ledger: &Ledger) -> Result<String, StoreError> {
    serialize_store(ledger, WHAT)
}

/// # Errors
/// See [`StoreError`]; the reader is stricter than the TypeScript one (`docs/PARITY.md`).
pub fn deserialize_ledger(json: &str) -> Result<Ledger, StoreError> {
    deserialize_store(json, WHAT, CURRENT_SCHEMA_VERSION)
}

/// `.debuggatha/ledger.json` inside the repository.
#[must_use]
pub fn ledger_file_path(repository_root: &Path) -> PathBuf {
    repository_root.join(".debuggatha").join("ledger.json")
}

/// A repository with no ledger yet is not an error: it has an empty history.
///
/// # Errors
/// See [`StoreError`].
pub fn load_ledger(repository_root: &Path, env: &dyn Env) -> Result<Ledger, StoreError> {
    match read_if_exists(&ledger_file_path(repository_root), WHAT)? {
        Some(text) => deserialize_ledger(&text),
        None => Ok(create_empty_ledger(&repository_root.to_string_lossy(), env)),
    }
}

/// # Errors
/// I/O failures; the partial file is removed.
pub fn save_ledger(ledger: &Ledger) -> Result<(), StoreError> {
    let path = ledger_file_path(Path::new(&ledger.repository_root));
    write_atomically(&path, &serialize_ledger(ledger)?, WHAT)
}
