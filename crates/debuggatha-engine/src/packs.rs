//! The Review Packs this engine ships, authored as TOML in `data/packs` and read as the same
//! `ReviewPack` the knowledge system takes.

use debuggatha_core::knowledge_system::ReviewPack;

const SOURCES: [&str; 43] = [
    include_str!("../data/packs/accessibility.toml"),
    include_str!("../data/packs/architecture.toml"),
    include_str!("../data/packs/aspnetcore.toml"),
    include_str!("../data/packs/astro.toml"),
    include_str!("../data/packs/bun.toml"),
    include_str!("../data/packs/cpp.toml"),
    include_str!("../data/packs/csharp.toml"),
    include_str!("../data/packs/dart.toml"),
    include_str!("../data/packs/dx.toml"),
    include_str!("../data/packs/electron.toml"),
    include_str!("../data/packs/elixir.toml"),
    include_str!("../data/packs/django.toml"),
    include_str!("../data/packs/elysia.toml"),
    include_str!("../data/packs/engineering.toml"),
    include_str!("../data/packs/express.toml"),
    include_str!("../data/packs/fastify.toml"),
    include_str!("../data/packs/flutter.toml"),
    include_str!("../data/packs/go.toml"),
    include_str!("../data/packs/flask.toml"),
    include_str!("../data/packs/hono.toml"),
    include_str!("../data/packs/java.toml"),
    include_str!("../data/packs/javascript.toml"),
    include_str!("../data/packs/kotlin.toml"),
    include_str!("../data/packs/laravel.toml"),
    include_str!("../data/packs/lit.toml"),
    include_str!("../data/packs/nodejs.toml"),
    include_str!("../data/packs/owasp.toml"),
    include_str!("../data/packs/performance.toml"),
    include_str!("../data/packs/phoenix.toml"),
    include_str!("../data/packs/php.toml"),
    include_str!("../data/packs/python.toml"),
    include_str!("../data/packs/rails.toml"),
    include_str!("../data/packs/react.toml"),
    include_str!("../data/packs/release.toml"),
    include_str!("../data/packs/ruby.toml"),
    include_str!("../data/packs/rust.toml"),
    include_str!("../data/packs/spring.toml"),
    include_str!("../data/packs/svelte.toml"),
    include_str!("../data/packs/swift.toml"),
    include_str!("../data/packs/tauri.toml"),
    include_str!("../data/packs/testing.toml"),
    include_str!("../data/packs/typescript.toml"),
    include_str!("../data/packs/vue.toml"),
];

/// The packs shipped with the engine.
///
/// # Panics
/// If a pack file does not parse; a test keeps them valid.
#[must_use]
// A pack that does not parse is a build defect, and a test reads each one; silence would read as "clean".
#[allow(clippy::expect_used)]
pub fn builtin_packs() -> Vec<ReviewPack> {
    SOURCES
        .iter()
        // debuggatha-ignore-next-line no-unwrap-expect -- embedded table, proven by the tests
        .map(|text| toml::from_str(text).expect("data/packs is valid"))
        .collect()
}

#[cfg(test)]
mod tests {
    use debuggatha_core::knowledge_system::validate_review_pack;

    use super::builtin_packs;
    use crate::detect::default_registry;

    /// The packs written for this engine: each of their rules has a detector.
    const WITH_DETECTORS: [&str; 13] = [
        "python",
        "java",
        "csharp",
        "swift",
        "elixir",
        "ruby",
        "cpp",
        "engineering",
        "django",
        "flask",
        "spring",
        "rails",
        "phoenix",
    ];

    #[test]
    fn every_pack_is_valid_and_the_engine_packs_have_a_detector_per_rule() {
        let registry = default_registry();
        for pack in builtin_packs() {
            assert_eq!(validate_review_pack(&pack), Ok(()), "{}", pack.id);
            let own = WITH_DETECTORS
                .iter()
                .any(|n| pack.id == format!("debuggatha/{n}"));
            for rule in pack.rules.iter().filter(|_| own) {
                assert!(registry.has(&rule.id), "{} has no detector", rule.id);
            }
        }
    }
}
