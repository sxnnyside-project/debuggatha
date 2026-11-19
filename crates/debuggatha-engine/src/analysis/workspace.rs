//! The files of a repository as the analysis sees them: every path the project owns, and the
//! content of those it must read (sources and manifests). Nothing here touches the disk after
//! [`Workspace::from_dir`], so every stage is a function of this value and can be tested with a
//! few lines of text.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use debuggatha_core::js::utf16_cmp;

/// Extensions whose imports the graph reads.
pub const ANALYZED_EXTENSIONS: &[&str] = &[
    "ts", "tsx", "mts", "cts", "js", "jsx", "mjs", "cjs", "vue", "svelte", "astro", "rs", "go",
    "py", "dart", "php", "ex", "exs", "java", "kt", "cs", "rb", "swift",
];

/// A source or manifest bigger than this is data, not code or configuration.
const MAX_READ_BYTES: u64 = 4 * 1024 * 1024;

/// Does the analysis read this file's content?
fn is_read(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path);
    let ext = name.rsplit_once('.').map(|(_, e)| e);
    ext.is_some_and(|e| ANALYZED_EXTENSIONS.contains(&e))
        || name == "package.json"
        || name == "Cargo.toml"
        || super::ecosystem::descriptors().iter().any(|(_, d)| {
            d.manifest
                .strip_prefix('*')
                .map_or(name == d.manifest, |e| name.ends_with(e))
        })
        || ((name.starts_with("tsconfig") || name.starts_with("jsconfig")) && ext == Some("json"))
}

/// Where a platform looks for the ownership file, in the order it prefers them.
pub const CODEOWNERS_PLACES: &[&str] = &[".github/CODEOWNERS", "CODEOWNERS", "docs/CODEOWNERS"];

/// Is the path a Rust source file?
#[must_use]
pub fn is_rust_file(path: &str) -> bool {
    path.rsplit_once('.').is_some_and(|(_, e)| e == "rs")
}

#[derive(Debug, Clone, Default)]
pub struct Workspace {
    /// Files a review owns: they belong to a boundary and are read.
    owned: BTreeSet<String>,
    /// Files that exist, including the ones a review skips by name (`.d.ts`).
    present: BTreeSet<String>,
    contents: BTreeMap<String, String>,
    /// Owned files by file name, and owned directories by their last component.
    by_name: BTreeMap<String, Vec<String>>,
    dirs: BTreeMap<String, BTreeSet<String>>,
}

impl Workspace {
    /// A workspace made of the given files, all owned and all readable (tests, and callers that
    /// hold the content already).
    pub fn from_files<I, P, C>(files: I) -> Self
    where
        I: IntoIterator<Item = (P, C)>,
        P: Into<String>,
        C: Into<String>,
    {
        let mut ws = Self::default();
        for (path, content) in files {
            let path = path.into();
            ws.owned.insert(path.clone());
            ws.present.insert(path.clone());
            ws.contents.insert(path, content.into());
        }
        ws.index();
        ws
    }

    /// Reads a directory the way a review scans it (`scan_scope`): git's file list or a walk, minus
    /// dependency, build and hidden directories.
    #[must_use]
    pub fn from_dir(root: &Path) -> Self {
        let mut ws = Self::default();
        let owned: BTreeSet<String> = crate::scan_scope::list_owned_files(root)
            .into_iter()
            .collect();
        for path in crate::scan_scope::list_tree_files(root) {
            if owned.contains(&path) && is_read(&path) {
                let full = root.join(&path);
                let small = std::fs::metadata(&full).is_ok_and(|m| m.len() <= MAX_READ_BYTES);
                if small && let Ok(bytes) = std::fs::read(&full) {
                    ws.contents
                        .insert(path.clone(), String::from_utf8_lossy(&bytes).into_owned());
                }
            }
            if owned.contains(&path) {
                ws.owned.insert(path.clone());
            }
            ws.present.insert(path);
        }
        // The ownership file lives where the tree listing does not look (`.github/`).
        for place in CODEOWNERS_PLACES {
            let full = root.join(place);
            if full
                .metadata()
                .is_ok_and(|m| m.is_file() && m.len() <= MAX_READ_BYTES)
                && let Ok(bytes) = std::fs::read(&full)
            {
                ws.contents.insert(
                    (*place).to_owned(),
                    String::from_utf8_lossy(&bytes).into_owned(),
                );
                ws.present.insert((*place).to_owned());
            }
        }
        ws.index();
        ws
    }

    fn index(&mut self) {
        for path in &self.owned {
            let name = path.rsplit('/').next().unwrap_or(path);
            self.by_name
                .entry(name.to_owned())
                .or_default()
                .push(path.clone());
            let mut dir = dirname(path);
            while !dir.is_empty() {
                let last = dir.rsplit('/').next().unwrap_or(dir);
                self.dirs
                    .entry(last.to_owned())
                    .or_default()
                    .insert(dir.to_owned());
                dir = dirname(dir);
            }
        }
    }

    /// Is there a file at `path`, owned by a review or not?
    #[must_use]
    pub fn exists(&self, path: &str) -> bool {
        self.present.contains(path)
    }

    /// The content of an owned file the analysis reads.
    #[must_use]
    pub fn read(&self, path: &str) -> Option<&str> {
        self.contents.get(path).map(String::as_str)
    }

    /// An owned file whose path is `suffix` or ends with `/suffix`.
    #[must_use]
    pub fn file_ending(&self, suffix: &str) -> Option<String> {
        let name = suffix.rsplit('/').next().unwrap_or(suffix);
        self.by_name
            .get(name)?
            .iter()
            .find(|p| **p == suffix || p.ends_with(&format!("/{suffix}")))
            .cloned()
    }

    /// An owned file with one of the extensions, directly inside a directory whose path is
    /// `suffix` or ends with `/suffix`.
    #[must_use]
    pub fn file_in_dir_ending(&self, suffix: &str, extensions: &[&str]) -> Option<String> {
        let name = suffix.rsplit('/').next().unwrap_or(suffix);
        self.dirs
            .get(name)?
            .iter()
            .filter(|d| **d == suffix || d.ends_with(&format!("/{suffix}")))
            .find_map(|dir| {
                self.owned
                    .range(format!("{dir}/")..)
                    .take_while(|p| p.starts_with(&format!("{dir}/")))
                    .find(|p| {
                        p[dir.len() + 1..]
                            .rsplit_once('.')
                            .is_some_and(|(stem, e)| !stem.contains('/') && extensions.contains(&e))
                    })
                    .cloned()
            })
    }

    /// The owned files a manifest name picks: the exact file name, or `*.ext` for any file with
    /// that extension.
    #[must_use]
    pub fn files_named(&self, manifest: &str) -> Vec<&str> {
        self.owned
            .iter()
            .map(String::as_str)
            .filter(|p| {
                let file = p.rsplit('/').next().unwrap_or(p);
                manifest
                    .strip_prefix('*')
                    .map_or(file == manifest, |ext| file.ends_with(ext))
            })
            .collect()
    }

    /// Every owned file, sorted.
    pub fn owned_files(&self) -> impl Iterator<Item = &str> {
        self.owned.iter().map(String::as_str)
    }

    /// The owned files with one of the extensions, sorted by UTF-16 code unit.
    #[must_use]
    pub fn files_with(&self, extensions: &[&str]) -> Vec<&str> {
        let mut out: Vec<&str> = self
            .owned
            .iter()
            .map(String::as_str)
            .filter(|p| {
                p.rsplit('/')
                    .next()
                    .and_then(|n| n.rsplit_once('.'))
                    .is_some_and(|(_, e)| extensions.contains(&e))
            })
            .collect();
        out.sort_by(|a, b| utf16_cmp(a, b));
        out
    }

    /// The names of the directories directly under `dir` (`""` is the root) that hold a file.
    #[must_use]
    pub fn child_dirs(&self, dir: &str) -> Vec<String> {
        let prefix = if dir.is_empty() {
            String::new()
        } else {
            format!("{dir}/")
        };
        let mut out = BTreeSet::new();
        for path in self.owned.range(prefix.clone()..) {
            let Some(rest) = path.strip_prefix(&prefix) else {
                break;
            };
            if let Some((child, _)) = rest.split_once('/') {
                out.insert(child.to_owned());
            }
        }
        out.into_iter().collect()
    }

    /// Does any owned file live under `dir`?
    #[must_use]
    pub fn has_dir(&self, dir: &str) -> bool {
        let prefix = format!("{dir}/");
        self.owned
            .range(prefix.clone()..)
            .next()
            .is_some_and(|p| p.starts_with(&prefix))
    }
}

/// `a/b/../c` → `a/c`, `./a` → `a`. A path that climbs out of the root keeps its `..`.
#[must_use]
pub fn normalize(path: &str) -> String {
    let mut out: Vec<&str> = Vec::new();
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." if out.last().is_some_and(|l| *l != "..") => {
                out.pop();
            }
            p => out.push(p),
        }
    }
    out.join("/")
}

/// The directory part of a path (`a/b/c.ts` gives `a/b`, `c.ts` gives the empty string).
#[must_use]
pub fn dirname(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(d, _)| d)
}

/// `join("a/b", "../c")` → `a/c`.
#[must_use]
pub fn join(dir: &str, rel: &str) -> String {
    if dir.is_empty() {
        normalize(rel)
    } else {
        normalize(&format!("{dir}/{rel}"))
    }
}
