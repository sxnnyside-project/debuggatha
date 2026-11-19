//! Turns the paths a tool prints into repository-relative ones.

use std::path::{Path, PathBuf};

/// A path or `file://` URI as a tool printed it, made repository-relative with `/` separators. A
/// path outside the repository, or one that climbs out with `..`, has nothing to attach to: `None`.
#[must_use]
pub fn to_repository_path(root: &Path, raw: &str) -> Option<String> {
    let path = match raw.strip_prefix("file://") {
        Some(rest) => percent_decode(rest.strip_prefix("localhost").unwrap_or(rest))?,
        None if raw.starts_with("file:") => percent_decode(&raw["file:".len()..])?,
        None => raw.to_owned(),
    };
    let path = Path::new(&path);
    let relative: PathBuf = if path.is_absolute() {
        let root = real(root);
        real(path).strip_prefix(&root).ok()?.to_path_buf()
    } else {
        path.to_path_buf()
    };
    let mut parts: Vec<&str> = Vec::new();
    for component in relative.components() {
        match component {
            std::path::Component::Normal(part) => parts.push(part.to_str()?),
            std::path::Component::CurDir => {}
            _ => return None,
        }
    }
    (!parts.is_empty()).then(|| parts.join("/"))
}

/// The path with symlinks resolved as far as it exists; a file a tool names may be gone, but the
/// directory it was in still resolves.
fn real(path: &Path) -> PathBuf {
    let mut rest = Vec::new();
    let mut current = path;
    loop {
        if let Ok(resolved) = current.canonicalize() {
            return rest.iter().rev().fold(resolved, |p, name| p.join(name));
        }
        match (current.parent(), current.file_name()) {
            (Some(parent), Some(name)) => {
                rest.push(name);
                current = parent;
            }
            _ => return path.to_path_buf(),
        }
    }
}

fn percent_decode(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = text.get(i + 1..i + 3)?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_become_relative_and_nothing_climbs_out() {
        let root = std::env::temp_dir();
        let inside = root.join("a").join("b.rs");
        let uri = format!("file://{}", inside.display());
        assert_eq!(to_repository_path(&root, &uri).as_deref(), Some("a/b.rs"));
        assert_eq!(
            to_repository_path(&root, "./src/x.js").as_deref(),
            Some("src/x.js")
        );
        assert_eq!(
            to_repository_path(&root, "src/a%20b.js"),
            Some("src/a%20b.js".to_owned())
        );
        assert_eq!(
            to_repository_path(&root, "file:///x/my%20dir/f.js"),
            None,
            "outside the root"
        );
        for outside in ["../x", "a/../../x", "/etc/passwd", "", "."] {
            assert_eq!(to_repository_path(&root, outside), None, "{outside}");
        }
    }
}
