//! Who can see a declaration, from the modifiers the language writes or from the name it uses.
//! `data/visibility.toml` holds the rule of each language.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use serde::Deserialize;

use crate::syntax::Language;

/// Who can see a declaration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Visibility {
    /// Anything that imports the module.
    Public,
    /// Anything in the repository, and not part of what it offers to others.
    Module,
    /// The package or directory that holds it.
    Package,
    /// The file.
    Private,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Rule {
    by_name: Option<String>,
    default: Option<String>,
    member_default: Option<String>,
    #[serde(default)]
    module: Vec<String>,
    #[serde(default)]
    wired: Vec<String>,
    #[serde(default)]
    public: Vec<String>,
    #[serde(default)]
    private: Vec<String>,
}

#[allow(clippy::expect_used)] // a table that does not parse is a build defect, caught by a test
fn rules() -> &'static BTreeMap<String, Rule> {
    static RULES: OnceLock<BTreeMap<String, Rule>> = OnceLock::new();
    RULES.get_or_init(|| {
        toml::from_str(include_str!("../../data/visibility.toml"))
            // debuggatha-ignore-next-line no-unwrap-expect -- embedded table, proven by the tests
            .expect("data/visibility.toml is valid")
    })
}

/// The visibility of a declaration called `name`, written with `modifiers` (the keywords or the
/// text of its modifier nodes, empty when there are none), and whether it is a member of a class.
#[must_use]
pub fn visibility(language: Language, name: &str, modifiers: &str, member: bool) -> Visibility {
    let Some(rule) = rules().get(language.id()) else {
        return Visibility::Public;
    };
    let first = name.chars().next();
    match rule.by_name.as_deref() {
        Some("capitalized") => {
            return if first.is_some_and(char::is_uppercase) {
                Visibility::Public
            } else {
                Visibility::Package
            };
        }
        Some("underscore-module") if first == Some('_') => return Visibility::Module,
        Some("underscore-package") if first == Some('_') => return Visibility::Package,
        Some(_) => return Visibility::Public,
        None => {}
    }
    let words: Vec<&str> = modifiers.split_whitespace().collect();
    let has = |list: &[String]| words.iter().any(|w| list.iter().any(|l| l == w));
    if has(&rule.public) {
        Visibility::Public
    } else if has(&rule.private) {
        Visibility::Private
    } else if has(&rule.module) {
        Visibility::Module
    } else {
        let default = if member {
            rule.member_default.as_ref().or(rule.default.as_ref())
        } else {
            rule.default.as_ref()
        };
        match default.map(String::as_str) {
            Some("package") => Visibility::Package,
            Some("module") => Visibility::Module,
            Some("private") => Visibility::Private,
            _ => Visibility::Public,
        }
    }
}

/// Does the table say who can see a declaration of the language? Without a rule every declaration
/// would read as public, so a language that has none is not read.
#[must_use]
pub fn reads(language: Language) -> bool {
    rules().contains_key(language.id())
}

/// Does a modifier say the declaration is reached by dispatch (`override`, `abstract`)?
#[must_use]
pub fn is_wired(language: Language, modifiers: &str) -> bool {
    rules().get(language.id()).is_some_and(|rule| {
        modifiers
            .split_whitespace()
            .any(|w| rule.wired.iter().any(|l| l == w))
    })
}

#[cfg(test)]
mod tests {
    use super::{Visibility, rules, visibility};
    use crate::syntax::Language;

    #[test]
    fn the_table_names_known_languages() {
        for id in rules().keys() {
            assert!(Language::ALL.iter().any(|l| l.id() == id), "{id}");
        }
    }

    #[test]
    fn the_name_or_the_modifier_decides() {
        assert_eq!(
            visibility(Language::Go, "Serve", "", false),
            Visibility::Public
        );
        assert_eq!(
            visibility(Language::Go, "serve", "", false),
            Visibility::Package
        );
        assert_eq!(
            visibility(Language::Dart, "_x", "", false),
            Visibility::Package
        );
        assert_eq!(
            visibility(Language::Java, "f", "public static", true),
            Visibility::Public
        );
        assert_eq!(
            visibility(Language::Java, "f", "", true),
            Visibility::Package
        );
        assert_eq!(
            visibility(Language::Java, "f", "private", true),
            Visibility::Private
        );
        assert_eq!(
            visibility(Language::Kotlin, "f", "", false),
            Visibility::Public
        );
        assert_eq!(
            visibility(Language::Elixir, "f", "defp", true),
            Visibility::Private
        );
        assert_eq!(
            visibility(Language::Elixir, "f", "def", true),
            Visibility::Public
        );
    }
}
