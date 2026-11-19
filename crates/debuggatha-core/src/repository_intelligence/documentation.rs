//! Documentation Context: which documents the repository has and what they say.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use indexmap::IndexMap;
use regress::Regex;

use super::fs::{list_dir, read_text};
use super::types::{
    DirListing, DocumentationKind, DocumentationProfile, DocumentationSource, ScanError, ScanResult,
};
use crate::js::{is_js_whitespace, js_trim};
use crate::knowledge_system::locale_compare;

const MAX_DEPTH: usize = 4;

const ROOT_DOC_FILES: [(&str, DocumentationKind); 5] = [
    ("README.md", DocumentationKind::Readme),
    ("CLAUDE.md", DocumentationKind::ClaudeMemory),
    ("CONTRIBUTING.md", DocumentationKind::Contributing),
    ("ARCHITECTURE.md", DocumentationKind::Architecture),
    ("ROADMAP.md", DocumentationKind::Roadmap),
];

const IGNORED_DIRS: [&str; 7] = [
    "node_modules",
    ".git",
    "dist",
    ".turbo",
    "target",
    "build",
    "legacy",
];

/// Kind from the file name and its directory: an ADR is anything under a path containing `adr` or
/// named `NNNN-...`. Substring matching, as in the TypeScript engine.
fn classify(file_name: &str, relative_dir: &str) -> DocumentationKind {
    let lower = file_name.to_lowercase();
    let starts_with_number = {
        let b = lower.as_bytes();
        b.len() >= 5 && b[..4].iter().all(u8::is_ascii_digit) && matches!(b[4], b'-' | b'_')
    };
    if relative_dir.to_lowercase().contains("adr") || starts_with_number {
        DocumentationKind::Adr
    } else if lower.contains("prd") {
        DocumentationKind::Prd
    } else if lower.contains("roadmap") {
        DocumentationKind::Roadmap
    } else if lower.contains("architecture") {
        DocumentationKind::Architecture
    } else if lower.contains("contributing") {
        DocumentationKind::Contributing
    } else if lower.starts_with("readme") {
        DocumentationKind::Readme
    } else if lower == "claude.md" {
        DocumentationKind::ClaudeMemory
    } else {
        DocumentationKind::Other
    }
}

/// `content.match(/^#\s+(.+)$/m)?.[1]?.trim()`, with the ECMAScript regex engine so that line
/// terminators and `\s` behave as they do in JavaScript.
fn extract_title(content: &str) -> Option<String> {
    let regex = Regex::with_flags(r"^#\s+(.+)$", "m").ok()?;
    let range = regex.find(content)?.group(1)?;
    Some(js_trim(&content[range]).to_owned())
}

/// `trimmed.split(/\s+/).length`: the runs of white space inside the trimmed text, plus one.
fn count_words(content: &str) -> u64 {
    let trimmed = js_trim(content);
    if trimmed.is_empty() {
        return 0;
    }
    let mut count = 1;
    let mut in_space = false;
    for c in trimmed.chars() {
        let space = is_js_whitespace(c);
        if space && !in_space {
            count += 1;
        }
        in_space = space;
    }
    count
}

fn build_summary(sources: &[DocumentationSource]) -> String {
    if sources.is_empty() {
        return "No documentation sources found.".to_owned();
    }
    let mut counts: IndexMap<&str, usize> = IndexMap::new();
    for source in sources {
        *counts.entry(source.kind.as_str()).or_insert(0) += 1;
    }
    let parts: Vec<String> = counts
        .iter()
        .map(|(kind, count)| format!("{count} {kind}{}", if *count > 1 { "s" } else { "" }))
        .collect();
    format!(
        "{} documentation source{} found: {}.",
        sources.len(),
        if sources.len() > 1 { "s" } else { "" },
        parts.join(", ")
    )
}

/// # Errors
/// An unreadable file or directory (including a broken symbolic link under `docs/`).
pub fn scan_documentation(
    root: &Path,
    root_entries: &[String],
) -> Result<ScanResult<DocumentationProfile>, ScanError> {
    let mut files_read: Vec<PathBuf> = Vec::new();
    let mut dirs_listed: Vec<DirListing> = Vec::new();
    let mut sources: Vec<DocumentationSource> = Vec::new();

    for (file, kind) in ROOT_DOC_FILES {
        if root_entries.iter().any(|e| e == file) {
            let path = root.join(file);
            files_read.push(path.clone());
            let content = read_text(&path)?;
            sources.push(DocumentationSource {
                file: file.to_owned(),
                kind,
                title: extract_title(&content),
                word_count: count_words(&content),
            });
        }
    }
    if root_entries.iter().any(|e| e == "docs") {
        walk(
            &root.join("docs"),
            "docs",
            0,
            &mut files_read,
            &mut dirs_listed,
            &mut sources,
        )?;
    }

    sources.sort_by(|a, b| locale_compare(&a.file, &b.file));
    let summary = build_summary(&sources);
    Ok(ScanResult {
        profile: DocumentationProfile { sources, summary },
        files_read,
        dirs_listed,
    })
}

fn walk(
    dir: &Path,
    relative_dir: &str,
    depth: usize,
    files_read: &mut Vec<PathBuf>,
    dirs_listed: &mut Vec<DirListing>,
    sources: &mut Vec<DocumentationSource>,
) -> Result<(), ScanError> {
    if depth > MAX_DEPTH {
        return Ok(());
    }
    let entries = list_dir(dir)?;
    dirs_listed.push(DirListing {
        dir: dir.to_path_buf(),
        entries: entries.clone(),
    });
    let ignored: HashSet<&str> = IGNORED_DIRS.into_iter().collect();
    for entry in &entries {
        if ignored.contains(entry.as_str()) {
            continue;
        }
        let full = dir.join(entry);
        // `statSync` follows symbolic links and throws on a broken one, so this does too.
        let metadata = std::fs::metadata(&full).map_err(|source| ScanError::Io {
            path: full.clone(),
            source,
        })?;
        if metadata.is_dir() {
            walk(
                &full,
                &format!("{relative_dir}/{entry}"),
                depth + 1,
                files_read,
                dirs_listed,
                sources,
            )?;
        } else if entry.to_lowercase().ends_with(".md") {
            files_read.push(full.clone());
            let content = read_text(&full)?;
            sources.push(DocumentationSource {
                file: format!("{relative_dir}/{entry}"),
                kind: classify(entry, relative_dir),
                title: extract_title(&content),
                word_count: count_words(&content),
            });
        }
    }
    Ok(())
}
