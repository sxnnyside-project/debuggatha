//! File access for the scanners: Node's behavior, reproduced.

use std::path::Path;

use super::types::ScanError;
use crate::js::utf16_cmp;

/// `readFileSync(path, 'utf8')`: invalid bytes become U+FFFD instead of failing.
pub(super) fn read_text(path: &Path) -> Result<String, ScanError> {
    std::fs::read(path)
        .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
        .map_err(|source| ScanError::Io {
            path: path.to_path_buf(),
            source,
        })
}

/// `[...readdirSync(dir)].sort()`: names in UTF-16 order. A name that is not valid Unicode is
/// shown with replacement characters (Node does the same).
pub(super) fn list_dir(dir: &Path) -> Result<Vec<String>, ScanError> {
    let read = std::fs::read_dir(dir).map_err(|source| ScanError::Io {
        path: dir.to_path_buf(),
        source,
    })?;
    let mut names = Vec::new();
    for entry in read {
        let entry = entry.map_err(|source| ScanError::Io {
            path: dir.to_path_buf(),
            source,
        })?;
        names.push(entry.file_name().to_string_lossy().into_owned());
    }
    names.sort_by(|a, b| utf16_cmp(a, b));
    Ok(names)
}
