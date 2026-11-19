//! A concept is something a reviewer looks for (`dynamic-eval`, `panic`); the table in
//! `data/concepts.toml` says which calls mean it in each language. A language missing from a
//! concept is not implemented; one marked `not_applicable` has no such thing.

use std::collections::{BTreeMap, HashMap};
use std::sync::OnceLock;

use serde::Deserialize;

use crate::syntax::Language;

/// What a concept means in one language: the calls that express it, or that it does not apply.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Meaning {
    /// The calls, as the language writes them.
    #[serde(default)]
    pub calls: Vec<String>,
    /// Whether the concept does not apply to the language, as opposed to not being written yet.
    #[serde(default)]
    pub not_applicable: bool,
    /// File names where the concept is how the program works (a build script panics to fail).
    #[serde(default)]
    pub skip_files: Vec<String>,
    /// Where each call without a wildcard sits in `calls`, by name.
    #[serde(skip)]
    exact: HashMap<String, usize>,
    /// The entries that match by pattern: position in `calls`, and the text a match starts and
    /// ends with, which rules out nearly every call before the full comparison.
    #[serde(skip)]
    patterns: Vec<(usize, String, String)>,
}

impl Meaning {
    fn index(&mut self) {
        for (at, call) in self.calls.iter().enumerate() {
            let name = call.split_once('@').map_or(call.as_str(), |(n, _)| n);
            if name.starts_with("Class.") {
                self.patterns.push((at, String::new(), String::new()));
            } else if let Some((head, _)) = name.split_once('*') {
                let tail = name.rsplit('*').next().unwrap_or_default();
                self.patterns.push((at, head.to_owned(), tail.to_owned()));
            } else {
                self.exact.entry(name.to_owned()).or_insert(at);
            }
        }
    }
}

#[derive(Debug, Deserialize)]
struct Definition {
    #[serde(default)]
    skip_tests: bool,
    /// Directories (as a path segment) where the concept is how the program works: output in a
    /// script or an example, `env()` in a configuration directory.
    #[serde(default)]
    skip_dirs: Vec<String>,
    #[serde(flatten)]
    languages: BTreeMap<String, Meaning>,
}

/// The concept table in `data/concepts.toml`: which calls mean each concept in each language.
#[derive(Debug, Deserialize)]
#[serde(transparent)]
pub struct Concepts(BTreeMap<String, Definition>);

/// Whether a concept is bound in a language.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// The language has an entry for it.
    Bound,
    /// It does not apply to the language.
    NotApplicable,
    /// The table has no entry yet; reported as a gap, never as clean.
    NotImplemented,
}

impl Concepts {
    /// What `concept` means in `language`, when the table says.
    #[must_use]
    pub fn meaning(&self, concept: &str, language: Language) -> Option<&Meaning> {
        self.0.get(concept)?.languages.get(key(language))
    }

    /// Whether `concept` is bound, not applicable or not implemented in `language`.
    #[must_use]
    pub fn status(&self, concept: &str, language: Language) -> Status {
        match self.meaning(concept, language) {
            None => Status::NotImplemented,
            Some(b) if b.not_applicable => Status::NotApplicable,
            Some(_) => Status::Bound,
        }
    }

    /// Is the path under a directory the concept leaves out?
    #[must_use]
    pub fn skips_dir(&self, concept: &str, file: &str) -> bool {
        self.0.get(concept).is_some_and(|d| {
            d.skip_dirs
                .iter()
                .any(|dir| file.split('/').rev().skip(1).any(|segment| segment == dir))
        })
    }

    /// Does the concept exclude test code?
    #[must_use]
    pub fn skips_tests(&self, concept: &str) -> bool {
        self.0.get(concept).is_some_and(|d| d.skip_tests)
    }

    /// Is `callee` one of the calls bound to the concept?
    #[must_use]
    pub fn is(&self, concept: &str, language: Language, callee: &str) -> bool {
        self.entry(concept, language, callee).is_some()
    }

    /// Which argument of the call the concept is about (`name@1` in the table; the first by default).
    #[must_use]
    pub fn argument(&self, concept: &str, language: Language, callee: &str) -> Option<usize> {
        let entry = self.entry(concept, language, callee)?;
        Some(
            entry
                .split_once('@')
                .and_then(|(_, n)| n.parse().ok())
                .unwrap_or(0),
        )
    }

    fn entry(&self, concept: &str, language: Language, callee: &str) -> Option<&str> {
        let meaning = self.meaning(concept, language)?;
        let by_pattern = meaning
            .patterns
            .iter()
            .find(|(at, head, tail)| {
                let e = meaning.calls[*at].as_str();
                callee.starts_with(head.as_str())
                    && callee.ends_with(tail.as_str())
                    && matches_callee(e.split_once('@').map_or(e, |(name, _)| name), callee)
            })
            .map(|(at, ..)| *at);
        let at = match (meaning.exact.get(callee).copied(), by_pattern) {
            (Some(a), Some(b)) => a.min(b),
            (a, b) => a.or(b)?,
        };
        Some(meaning.calls[at].as_str())
    }
}

/// JavaScript and TypeScript share one table.
fn key(language: Language) -> &'static str {
    match language {
        Language::JavaScript
        | Language::Jsx
        | Language::TypeScript
        | Language::Tsx
        | Language::Vue
        | Language::Svelte
        | Language::Astro => "javascript",
        other => other.id(),
    }
}

/// `{a,b}` stands for each alternative: `{db,pool}.query` is `db.query` and `pool.query`.
fn expand(entry: &str) -> Vec<String> {
    let Some(open) = entry.find('{') else {
        return vec![entry.to_owned()];
    };
    let Some(close) = entry[open..].find('}').map(|c| c + open) else {
        return vec![entry.to_owned()];
    };
    entry[open + 1..close]
        .split(',')
        .flat_map(|alt| expand(&format!("{}{alt}{}", &entry[..open], &entry[close + 1..])))
        .collect()
}

/// `*` stands for any run of characters, so `*.find` is any receiver and `*Repository.save` any
/// repository.
fn matches_callee(entry: &str, callee: &str) -> bool {
    if let Some(method) = entry.strip_prefix("Class.") {
        // A class-like receiver (`User`, `OrderItem`) and a method only an ORM defines.
        let Some((receiver, name)) = callee.rsplit_once('.') else {
            return false;
        };
        let last = receiver.rsplit('.').next().unwrap_or(receiver);
        return name == method
            && last.chars().next().is_some_and(char::is_uppercase)
            && last.chars().any(char::is_lowercase);
    }
    let mut parts = entry.split('*');
    let first = parts.next().unwrap_or_default();
    let Some(mut rest) = callee.strip_prefix(first) else {
        return false;
    };
    let mut tail = None;
    for part in parts {
        if let Some(prev) = tail.replace(part) {
            match rest.find(prev) {
                Some(at) => rest = &rest[at + prev.len()..],
                None => return false,
            }
        }
    }
    tail.map_or(rest.is_empty(), |last| rest.ends_with(last))
}

/// The table shipped with the engine.
///
/// # Panics
/// If `data/concepts.toml` does not parse; a test keeps it valid.
#[must_use]
// A table that does not parse is a build defect, and a test parses it; silence would read as "clean".
#[allow(clippy::expect_used)]
pub fn concepts() -> &'static Concepts {
    static TABLE: OnceLock<Concepts> = OnceLock::new();
    TABLE.get_or_init(|| {
        let mut table: Concepts = toml::from_str(include_str!("../../data/concepts.toml"))
            // debuggatha-ignore-next-line no-unwrap-expect -- embedded table, proven by the tests
            .expect("data/concepts.toml is valid");
        for definition in table.0.values_mut() {
            for meaning in definition.languages.values_mut() {
                meaning.calls = meaning.calls.iter().flat_map(|c| expand(c)).collect();
                meaning.index();
            }
        }
        table
    })
}

#[cfg(test)]
mod tests {
    use super::{Status, concepts};
    use crate::syntax::Language;

    #[test]
    fn table_parses_and_names_known_languages() {
        let table = concepts();
        for definition in table.0.values() {
            for id in definition.languages.keys() {
                assert!(
                    id == "javascript" || Language::ALL.iter().any(|l| l.id() == id),
                    "unknown language {id}"
                );
            }
        }
    }

    #[test]
    fn a_missing_language_is_not_implemented_and_n_a_is_stated() {
        let table = concepts();
        assert_eq!(table.status("dynamic-eval", Language::Php), Status::Bound);
        assert_eq!(
            table.status("dynamic-eval", Language::Go),
            Status::NotApplicable
        );
        assert_eq!(
            table.status("dynamic-eval", Language::Kotlin),
            Status::NotImplemented
        );
    }

    #[test]
    fn a_wildcard_receiver_matches_any_receiver_only() {
        let table = concepts();
        assert!(table.is("data-query", Language::Ruby, "User.find_by"));
        assert!(!table.is("data-query", Language::Ruby, "User.find_by_name"));
        assert!(table.is("data-query", Language::Java, "UserRepository.findByName"));
        assert!(table.is("dynamic-eval", Language::Php, "eval"));
        assert!(!table.is("dynamic-eval", Language::Php, "x.eval"));
    }
}
