//! An in-process cache of the repository snapshot, invalidated by what it read.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use super::fs::list_dir;
use super::types::{DirListing, RepositoryContext};
use crate::knowledge_system::locale_compare;

/// The modification time and size of a file the context was built from.
#[derive(Debug, Clone, PartialEq)]
pub struct FileStamp {
    /// The file.
    pub path: PathBuf,
    /// Modification time, in milliseconds.
    pub mtime_ms: f64,
    /// Size, in bytes.
    pub size: u64,
}

/// What the context was built from. If any directory listing or file stamp changes, it is stale.
#[derive(Debug, Clone, PartialEq)]
pub struct Fingerprint {
    /// The directories that were listed.
    pub dirs: Vec<DirListing>,
    /// The files that were read.
    pub files: Vec<FileStamp>,
}

fn stamp(path: &Path) -> Option<FileStamp> {
    let metadata = std::fs::metadata(path).ok()?;
    let modified = metadata.modified().ok()?.duration_since(UNIX_EPOCH).ok()?;
    Some(FileStamp {
        path: path.to_path_buf(),
        mtime_ms: modified.as_secs_f64() * 1000.0,
        size: metadata.len(),
    })
}

/// # Panics
/// Never; a file that cannot be stamped is recorded with a zero stamp so it reads as changed.
#[must_use]
pub fn build_fingerprint(dirs: Vec<DirListing>, file_paths: &[PathBuf]) -> Fingerprint {
    let mut by_dir: HashMap<PathBuf, DirListing> = HashMap::new();
    for listing in dirs {
        by_dir.insert(listing.dir.clone(), listing);
    }
    let mut dirs: Vec<DirListing> = by_dir.into_values().collect();
    dirs.sort_by(|a, b| locale_compare(&a.dir.to_string_lossy(), &b.dir.to_string_lossy()));
    let mut unique: Vec<&PathBuf> = file_paths.iter().collect();
    unique.sort_by(|a, b| crate::js::utf16_cmp(&a.to_string_lossy(), &b.to_string_lossy()));
    unique.dedup();
    let files = unique
        .into_iter()
        .map(|p| {
            stamp(p).unwrap_or(FileStamp {
                path: p.clone(),
                mtime_ms: 0.0,
                size: 0,
            })
        })
        .collect();
    Fingerprint { dirs, files }
}

/// Whether anything the fingerprint recorded has changed on disk.
#[must_use]
pub fn has_fingerprint_changed(fingerprint: &Fingerprint) -> bool {
    for listing in &fingerprint.dirs {
        match list_dir(&listing.dir) {
            Ok(current) if current == listing.entries => {}
            _ => return true,
        }
    }
    fingerprint
        .files
        .iter()
        .any(|file| match stamp(&file.path) {
            Some(now) => now.mtime_ms.to_bits() != file.mtime_ms.to_bits() || now.size != file.size,
            None => true,
        })
}

/// A cached context and the fingerprint it was built from.
#[derive(Debug, Clone)]
pub struct CacheEntry {
    /// The cached context.
    pub context: RepositoryContext,
    /// What it was built from.
    pub fingerprint: Fingerprint,
}

/// An in-process cache of the repository snapshot. Different lifetime from the ledger, which
/// persists findings: this one lives and dies with the process.
#[derive(Debug, Default)]
pub struct RepositoryContextCache {
    entries: HashMap<String, CacheEntry>,
}

impl RepositoryContextCache {
    /// The entry for the repository at `root`.
    #[must_use]
    pub fn get(&self, root: &str) -> Option<&CacheEntry> {
        self.entries.get(root)
    }

    /// Store the entry for the repository at `root`.
    pub fn set(&mut self, root: &str, entry: CacheEntry) {
        self.entries.insert(root.to_owned(), entry);
    }
}
