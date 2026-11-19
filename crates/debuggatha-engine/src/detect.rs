//! Detectors: the code that turns "no-explicit-any" into an actual check. A rule is declarative
//! data; something has to read the code, and this registry is that something. Coverage is
//! deliberately partial: a rule with no detector here is inspectable in the policy but produces no
//! findings, and a review must report it as not covered (CLAUDE.md: never fakes certainty).

pub mod coverage;
mod excerpt;
mod guidance;
mod hit;
mod input;
pub mod patterns;
pub mod queries;
pub mod rules;
mod runner;

use crate::syntax::Language;
pub use excerpt::{excerpt_of, redact_known_tokens};
pub use guidance::{Guidance, guidance_for};
pub use hit::{Hit, HitEdit};
pub use input::{DetectorInput, RepoFile, SourceUnit};
pub use runner::{
    Detector, GroupDetector, NotAnalyzed, Registry, Run, run_review, run_review_controlled,
    run_review_with_files,
};

/// Rules the TypeScript engine does not have: there is nothing to compare them with,
/// so the corpus gate counts them apart and their proof is fixtures, mutations and a reading of
/// their hits (`docs/DETECTOR_QUALITY.md`).
pub const NEW_RULES: &[&str] = &[
    "require-await",
    "no-loop-func",
    "no-floating-promises",
    "leak-mock-restore",
    "n-plus-one-queries",
    "a03-injection",
    "validate-input",
    "document-env-vars",
    "no-unsafe-deserialization",
    "no-weak-hash",
    "no-print-stack-trace",
    "no-fatal-error",
    "no-raw-markup",
    "no-permit-all",
    "no-atom-from-input",
    "no-unbounded-c-string",
    "no-debug-output",
    "no-weak-random",
    "a10-ssrf",
    "no-path-traversal",
    "no-env-outside-config",
    "async-all-the-way",
    "no-swallowed-error",
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
    "no-weak-cipher",
    "no-open-redirect",
    "no-template-injection",
    "no-unsafe-reflection",
    "no-regex-from-input",
    "no-unsafe-xml-parser",
    "no-process-exit",
    "no-type-punning",
    "no-http-without-timeout",
    "no-format-string",
    "no-extract",
    "no-removed-mysql-api",
    "no-global-scope",
    "no-run-blocking",
    "no-gc-collect",
    "no-dispatch-main-sync",
    "no-unfinished-code",
    "no-env-mutation",
    "no-leak-memory",
    "spring-csrf-disabled",
    "spring-permit-all-requests",
    "aspnet-developer-exception-page",
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
];

/// The languages in which a rule of the registry can report something.
#[must_use]
pub fn languages_checked(rule: &str) -> Vec<Language> {
    const SCRIPT: [Language; 7] = [
        Language::JavaScript,
        Language::Jsx,
        Language::TypeScript,
        Language::Tsx,
        Language::Vue,
        Language::Svelte,
        Language::Astro,
    ];
    let mut found = rules::concepts::languages_of(rule);
    found.extend(patterns::languages_of(rule));
    let in_group = |rules: &[&str]| rules.contains(&rule);
    if [
        rules::js::RULES,
        rules::calls::RULES,
        rules::functions::RULES,
        rules::promises::RULES,
        rules::leaks::RULES,
        rules::taint::RULES,
    ]
    .into_iter()
    .any(in_group)
    {
        found.extend(SCRIPT);
    }
    if rules::template::RULES.contains(&rule) {
        found.push(Language::Vue);
    }
    if matches!(rule, "no-unwrap-expect" | "no-unsafe-blocks") {
        found.push(Language::Rust);
    }
    if rule == "avoid-print" {
        found.push(Language::Dart);
    }
    if rules::env::RULES.contains(&rule) || rule == rules::secrets::RULE {
        found.extend(Language::ALL.into_iter().filter(|l| *l != Language::Html));
    }
    found.sort_by_key(|l| l.id());
    found.dedup();
    found
}

/// Every detector this engine ships. A rule that is not here is a gap the review reports.
#[must_use]
pub fn default_registry() -> Registry {
    let mut registry = Registry::new();
    rules::js::register(&mut registry);
    rules::calls::register(&mut registry);
    queries::register(&mut registry);
    patterns::register(&mut registry);
    rules::concepts::register(&mut registry);
    rules::template::register(&mut registry);
    rules::secrets::register(&mut registry);
    rules::functions::register(&mut registry);
    rules::promises::register(&mut registry);
    rules::leaks::register(&mut registry);
    rules::taint::register(&mut registry);
    rules::env::register(&mut registry);
    registry
}
