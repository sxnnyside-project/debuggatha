//! Ownership from `CODEOWNERS` and nothing else: a module whose files no rule covers is `unknown`,
//! never inferred from history. The matching is the one the file format has:
//! a pattern without a slash matches at any depth, `*` stays inside one path segment, `**` crosses
//! them, the last matching rule wins, and a rule with no owner takes ownership away. Ownership is
//! decided per file; a module reports the owners of its files and says when they disagree.

use serde::Serialize;

use super::structure::boundaries::ModuleBoundary;
use super::structure::workspace::{CODEOWNERS_PLACES, Workspace};

/// Where the owners of a module come from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum OwnershipSource {
    /// A `CODEOWNERS` file.
    Codeowners,
    /// Nowhere: no rule covers the module's files.
    Unknown,
}

/// Who owns a module, from `CODEOWNERS` only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OwnershipSignal {
    /// The module's id.
    pub module_id: String,
    /// The owners of the module's files, in the order the rules name them.
    pub owners: Vec<String>,
    /// Where the owners come from.
    pub source: OwnershipSource,
    /// Files of the module with at least one owner, and all its files.
    pub owned_files: usize,
    /// How many files the module has.
    pub total_files: usize,
    /// Every file of the module has the same owners.
    pub uniform: bool,
}

#[derive(Debug, Clone)]
struct Rule {
    segments: Vec<String>,
    anchored: bool,
    dir_only: bool,
    owners: Vec<String>,
}

fn parse_rules(content: &str) -> Vec<Rule> {
    let mut rules = Vec::new();
    for raw in content.lines() {
        let line = raw.trim();
        // `[Section]` headers (GitLab) carry no pattern of their own.
        if line.is_empty()
            || line.starts_with('#')
            || line.starts_with('[')
            || line.starts_with("^[")
        {
            continue;
        }
        let mut tokens = line.split_whitespace();
        let Some(pattern) = tokens.next() else {
            continue;
        };
        let owners: Vec<String> = tokens
            .take_while(|t| !t.starts_with('#'))
            .map(str::to_owned)
            .collect();
        let pattern = pattern.strip_prefix('\\').unwrap_or(pattern);
        let dir_only = pattern.ends_with('/');
        let body = pattern.trim_end_matches('/');
        let anchored = body.starts_with('/') || body.contains('/');
        let segments: Vec<String> = body
            .trim_start_matches('/')
            .split('/')
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
            .collect();
        if segments.is_empty() {
            continue;
        }
        rules.push(Rule {
            segments,
            anchored,
            dir_only,
            owners,
        });
    }
    rules
}

/// `*` and `?` inside one segment.
fn segment_matches(pattern: &[char], text: &[char]) -> bool {
    match pattern.split_first() {
        None => text.is_empty(),
        Some(('*', rest)) => (0..=text.len()).any(|i| segment_matches(rest, &text[i..])),
        Some(('?', rest)) => !text.is_empty() && segment_matches(rest, &text[1..]),
        Some((c, rest)) => text.first() == Some(c) && segment_matches(rest, &text[1..]),
    }
}

/// Every length of path prefix (in segments) the pattern segments can consume from `path`.
fn consume(pattern: &[String], path: &[&str], out: &mut Vec<usize>, at: usize) {
    let Some((head, rest)) = pattern.split_first() else {
        out.push(at);
        return;
    };
    if head == "**" {
        // A trailing `**` takes at least one segment (`foo/**` is what is inside `foo`).
        let least = usize::from(rest.is_empty());
        for take in least..=(path.len() - at) {
            consume(rest, path, out, at + take);
        }
        return;
    }
    if at < path.len() {
        let p: Vec<char> = head.chars().collect();
        let t: Vec<char> = path[at].chars().collect();
        if segment_matches(&p, &t) {
            consume(rest, path, out, at + 1);
        }
    }
}

impl Rule {
    fn matches(&self, path: &str) -> bool {
        let parts: Vec<&str> = path.split('/').collect();
        let starts: Vec<usize> = if self.anchored {
            vec![0]
        } else {
            (0..parts.len()).collect()
        };
        // A last segment with a wildcard names the files it matches, not the directories above
        // them (`docs/*` is not `docs/a/b.md`); a plain name also owns what is under it.
        let last_is_glob = self
            .segments
            .last()
            .is_some_and(|s| s.contains('*') || s.contains('?'));
        for start in starts {
            let mut ends = Vec::new();
            consume(&self.segments, &parts[start..], &mut ends, 0);
            for end in ends {
                let covers_all = start + end == parts.len();
                if covers_all && !self.dir_only {
                    return true;
                }
                if !covers_all && !last_is_glob {
                    return true;
                }
            }
        }
        false
    }
}

fn owners_of(rules: &[Rule], file: &str) -> Vec<String> {
    let mut owners = Vec::new();
    for rule in rules {
        if rule.matches(file) {
            owners.clone_from(&rule.owners);
        }
    }
    owners
}

/// The ownership of every boundary. `CODEOWNERS` is read where the platform looks for it,
/// `.github/` first.
#[must_use]
pub fn detect_ownership(ws: &Workspace, boundaries: &[ModuleBoundary]) -> Vec<OwnershipSignal> {
    let rules = CODEOWNERS_PLACES
        .iter()
        .find_map(|p| ws.read(p))
        .map(parse_rules)
        .unwrap_or_default();
    boundaries
        .iter()
        .map(|b| {
            // A module with no analyzed file (a Dart or Kotlin package, a folder of styles) is
            // owned the way a directory is: what the rules say of a path inside it.
            let per_file: Vec<Vec<String>> = if b.files.is_empty() {
                let inside = if b.id == "." {
                    "x".to_owned()
                } else {
                    format!("{}/x", b.id)
                };
                vec![owners_of(&rules, &inside)]
            } else {
                b.files.iter().map(|f| owners_of(&rules, f)).collect()
            };
            let mut owners: Vec<String> = Vec::new();
            for o in per_file.iter().flatten() {
                if !owners.contains(o) {
                    owners.push(o.clone());
                }
            }
            let owned_files = if b.files.is_empty() {
                0
            } else {
                per_file.iter().filter(|o| !o.is_empty()).count()
            };
            let uniform = per_file.windows(2).all(|w| w[0] == w[1]);
            OwnershipSignal {
                module_id: b.id.clone(),
                source: if owners.is_empty() {
                    OwnershipSource::Unknown
                } else {
                    OwnershipSource::Codeowners
                },
                owners,
                owned_files,
                total_files: b.files.len(),
                uniform,
            }
        })
        .collect()
}
