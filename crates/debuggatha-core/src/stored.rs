//! Reading and writing the versioned JSON files under `.debuggatha/` (ledger, memory).
//!
//! Shared so every stored file gets the same guarantees: sorted keys and two-space indentation,
//! fail-closed reading, and an atomic write.

use std::fmt;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

#[derive(Debug)]
pub struct StoreError {
    what: &'static str,
    kind: Kind,
}

#[derive(Debug)]
enum Kind {
    InvalidJson(String),
    MissingSchemaVersion,
    UnsupportedVersion { found: String, current: u32 },
    Malformed(String),
    Io(std::io::Error),
    Serialize(String),
}

impl StoreError {
    fn new(what: &'static str, kind: Kind) -> Self {
        Self { what, kind }
    }

    /// The file could not be opened, read or written, as opposed to its content being wrong.
    #[must_use]
    pub fn is_io(&self) -> bool {
        matches!(self.kind, Kind::Io(_))
    }
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let what = self.what;
        match &self.kind {
            Kind::InvalidJson(why) => write!(f, "Cannot read {what}: not valid JSON ({why})."),
            Kind::MissingSchemaVersion => {
                write!(
                    f,
                    "Cannot read {what}: missing or non-numeric schemaVersion."
                )
            }
            Kind::UnsupportedVersion { found, current } => write!(
                f,
                "Cannot read {what}: schema version {found} is not supported (current: {current}). \
                 No migration path exists yet for this version gap — this is where a future migration gets added."
            ),
            Kind::Malformed(why) => write!(f, "Cannot read {what}: {why}"),
            Kind::Io(error) => write!(f, "Cannot access the {what} file: {error}"),
            Kind::Serialize(why) => write!(f, "Cannot write {what}: {why}"),
        }
    }
}

impl std::error::Error for StoreError {}

/// Sorted keys, two-space indent, trailing newline: the same value always gives the same bytes.
///
/// # Errors
/// The value cannot be written as JSON.
pub fn serialize_store<T: Serialize>(store: &T, what: &'static str) -> Result<String, StoreError> {
    let fail = |error: serde_json::Error| StoreError::new(what, Kind::Serialize(error.to_string()));
    let value = sorted(serde_json::to_value(store).map_err(fail)?);
    let mut text = serde_json::to_string_pretty(&value).map_err(fail)?;
    text.push('\n');
    Ok(text)
}

/// Rebuilds every object with its keys in code-unit order, like `sortKeysDeep`.
fn sorted(value: Value) -> Value {
    match value {
        Value::Array(items) => Value::Array(items.into_iter().map(sorted).collect()),
        Value::Object(map) => {
            let mut entries: Vec<(String, Value)> = map.into_iter().collect();
            entries.sort_by(|a, b| a.0.encode_utf16().cmp(b.0.encode_utf16()));
            Value::Object(entries.into_iter().map(|(k, v)| (k, sorted(v))).collect())
        }
        other => other,
    }
}

/// Fails closed: invalid JSON, an unknown schema version, a wrong shape, or a field this version
/// does not know are all errors, so a save never drops what a
/// read did not understand (`docs/PARITY.md`).
///
/// # Errors
/// See [`StoreError`].
pub fn deserialize_store<T: Serialize + DeserializeOwned>(
    json: &str,
    what: &'static str,
    current_version: u32,
) -> Result<T, StoreError> {
    let err = |kind| StoreError::new(what, kind);
    let raw: Value =
        serde_json::from_str(json).map_err(|e| err(Kind::InvalidJson(e.to_string())))?;
    let Some(version) = raw.get("schemaVersion").and_then(Value::as_f64) else {
        return Err(err(Kind::MissingSchemaVersion));
    };
    if (version - f64::from(current_version)).abs() > f64::EPSILON {
        let found = if version.fract() == 0.0 {
            format!("{version:.0}")
        } else {
            version.to_string()
        };
        return Err(err(Kind::UnsupportedVersion {
            found,
            current: current_version,
        }));
    }
    let store: T =
        serde_json::from_value(raw.clone()).map_err(|e| err(Kind::Malformed(e.to_string())))?;
    if serde_json::to_value(&store).unwrap_or(Value::Null) != raw {
        return Err(err(Kind::Malformed(
            "the file holds fields or values this version does not understand; refusing to read it rather than drop them on the next save.".to_owned(),
        )));
    }
    Ok(store)
}

/// The text of `path`, or `None` when the file does not exist.
///
/// # Errors
/// Any I/O failure other than "not found".
pub fn read_if_exists(path: &Path, what: &'static str) -> Result<Option<String>, StoreError> {
    match fs::read_to_string(path) {
        Ok(text) => Ok(Some(text)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(StoreError::new(what, Kind::Io(error))),
    }
}

/// Write beside the file and rename, so a reader never sees half a file even if the process is
/// cancelled mid-write. The data is flushed to disk before the rename.
///
/// # Errors
/// I/O failures; the partial file is removed.
pub fn write_atomically(path: &Path, text: &str, what: &'static str) -> Result<(), StoreError> {
    let io = |error| StoreError::new(what, Kind::Io(error));
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(io)?;
    }
    let mut partial = path.as_os_str().to_owned();
    partial.push(format!(".{}.tmp", std::process::id()));
    let partial = PathBuf::from(partial);
    let write = || -> std::io::Result<()> {
        let mut file = fs::File::create(&partial)?;
        file.write_all(text.as_bytes())?;
        file.sync_all()?;
        fs::rename(&partial, path)
    };
    write().map_err(|error| {
        // The write error is the one worth reporting; a leftover partial file is not.
        drop(fs::remove_file(&partial));
        io(error)
    })
}

/// Removes `path`; `false` when there was nothing to remove.
///
/// # Errors
/// Any I/O failure other than "not found".
pub fn remove_if_exists(path: &Path, what: &'static str) -> Result<bool, StoreError> {
    match fs::remove_file(path) {
        Ok(()) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(StoreError::new(what, Kind::Io(error))),
    }
}
