//! Rules of one language written as a query (`data/patterns/*.scm`): the capture `@hit.<rule>`
//! marks the code to report. A rule that needs more than the shape of the tree belongs to a
//! concept or to its own detector.

use std::path::Path;

use super::hit::Hit;
use super::input::DetectorInput;
use super::rules::calls::one_per_line;
use super::rules::js::{Lines, make_hit};
use super::runner::Registry;
use crate::scan_scope::is_test_path;
use crate::syntax::{Language, QuerySet};
use debuggatha_core::review_engine::Confidence;

/// The rules this group answers for.
pub const RULES: &[&str] = &[
    "no-double-bang",
    "no-global-variables",
    "avoid-dynamic",
    "prefer-is-empty",
    "no-force-try",
    "no-force-cast",
    "no-force-unwrap",
    "no-raw-new-delete",
    "no-malloc-free",
    "no-using-namespace-in-header",
    "no-assert-in-production",
    "no-bare-except",
    "no-mutable-default-argument",
    "no-wildcard-import",
    "no-insecure-tls",
    "no-flask-debug",
    "no-catch-all",
    "no-permissive-file-mode",
    "no-finalize",
    "no-async-void",
    "no-lock-this",
    "no-unsafe-package",
    "no-pprof-import",
    "no-defer-in-loop",
    "no-static-mut",
    "no-error-suppression",
    "no-c-style-cast",
    "no-csrf-skip",
    "no-type-punning",
    "no-open-cors",
    "django-csrf-exempt",
    "django-debug-enabled",
    "django-secret-key-in-code",
    "django-allowed-hosts-wildcard",
    "django-cookie-not-secure",
    "flask-secret-key-in-code",
    "spring-field-injection",
    "rails-skip-authentication",
    "ecto-fragment-interpolation",
    "phoenix-check-origin-off",
    "aspnet-ignore-antiforgery",
    "aspnet-new-httpclient",
];

/// Rules that only mean something in some files, by extension.
/// A hit whose first line holds one of these is left out: an `async void` event handler.
const NOT_WHEN_HEADER_HAS: &[(&str, &[&str])] = &[("no-async-void", &["EventArgs", "override "])];

const ONLY_IN: &[(&str, &[&str])] = &[("no-using-namespace-in-header", &["h", "hpp", "hh", "hxx"])];

pub fn register(registry: &mut Registry) {
    registry.register_group(RULES, detect);
}

const SOURCES: [(Language, &str); 12] = [
    (
        Language::Kotlin,
        include_str!("../../data/patterns/kotlin.scm"),
    ),
    (Language::Php, include_str!("../../data/patterns/php.scm")),
    (Language::Dart, include_str!("../../data/patterns/dart.scm")),
    (
        Language::Swift,
        include_str!("../../data/patterns/swift.scm"),
    ),
    (Language::Cpp, include_str!("../../data/patterns/cpp.scm")),
    (
        Language::Elixir,
        include_str!("../../data/patterns/elixir.scm"),
    ),
    (
        Language::Python,
        include_str!("../../data/patterns/python.scm"),
    ),
    (Language::Java, include_str!("../../data/patterns/java.scm")),
    (
        Language::CSharp,
        include_str!("../../data/patterns/csharp.scm"),
    ),
    (Language::Go, include_str!("../../data/patterns/go.scm")),
    (Language::Rust, include_str!("../../data/patterns/rust.scm")),
    (Language::Ruby, include_str!("../../data/patterns/ruby.scm")),
];

static QUERIES: QuerySet<{ SOURCES.len() }> = QuerySet::new(SOURCES);

/// Why a query does not compile, if any (a test asserts there is none).
#[must_use]
pub fn compile_errors() -> Vec<String> {
    QUERIES.errors()
}

/// The languages whose patterns report the rule.
#[must_use]
pub fn languages_of(rule: &str) -> Vec<Language> {
    let marker = format!("@hit.{rule}");
    QUERIES
        .sources()
        .filter(|(_, source)| source.contains(&marker))
        .map(|(language, _)| language)
        .collect()
}

#[must_use]
pub fn detect(input: &DetectorInput<'_>) -> Vec<(&'static str, Hit)> {
    let Some(syntax) = input.syntax else {
        return Vec::new();
    };
    let Some(query) = QUERIES.query_for(syntax.language) else {
        return Vec::new();
    };
    let Ok(matches) = query.run(syntax, input.content) else {
        return Vec::new();
    };
    if is_test_path(input.file) {
        return Vec::new();
    }
    let lines = Lines::new(input.content);
    let mut hits = Vec::new();
    for m in &matches {
        for capture in &m.captures {
            let Some(rule) = capture
                .name
                .strip_prefix("hit.")
                .and_then(|name| RULES.iter().find(|r| **r == name))
            else {
                continue;
            };
            let extension = Path::new(input.file)
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or_default();
            if ONLY_IN
                .iter()
                .any(|(r, extensions)| r == rule && !extensions.contains(&extension))
            {
                continue;
            }
            let (start, end) = (capture.span.start as usize, capture.span.end as usize);
            let header = input.content.get(start..end).unwrap_or_default();
            let header = header.lines().next().unwrap_or_default();
            if NOT_WHEN_HEADER_HAS
                .iter()
                .any(|(r, words)| r == rule && words.iter().any(|w| header.contains(w)))
            {
                continue;
            }
            if let Some(hit) = make_hit(input, &lines, start, end, Confidence::High, None) {
                hits.push((*rule, hit, start));
            }
        }
    }
    one_per_line(hits)
}
