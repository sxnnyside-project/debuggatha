//! A repository with the manifest of a language gets the rules of that language's pack, and only
//! those.
#![allow(clippy::expect_used, clippy::unwrap_used)] // a failed expect is a failed test

use debuggatha_core::env::SystemEnv;
use debuggatha_core::knowledge_system::{
    CapabilityRegistry, PolicyRequest, ResolutionContext, RuleScope, resolve_policy,
};
use debuggatha_core::repository_intelligence::build_repository_context;
use debuggatha_engine::detect::languages_checked;
use debuggatha_engine::packs::builtin_packs;
use debuggatha_engine::syntax::Language;

fn rules_for(manifest: &str) -> Vec<String> {
    rules_for_content(manifest, "")
}

fn rules_for_content(manifest: &str, text: &str) -> Vec<String> {
    let root = std::env::temp_dir().join(format!(
        "debuggatha-packs-{}-{}",
        std::process::id(),
        manifest.replace('.', "_")
    ));
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join(manifest), text).unwrap();
    let context = build_repository_context(&root, &SystemEnv, None).unwrap();
    std::fs::remove_dir_all(&root).unwrap();
    let packs = builtin_packs();
    let request = PolicyRequest {
        requested_pack_ids: packs.iter().map(|p| p.id.clone()).collect(),
        requested_policy_id: None,
    };
    let registry = CapabilityRegistry::new(packs, vec![]);
    let policy = resolve_policy(
        &ResolutionContext::from_context(&context),
        &request,
        &registry,
    )
    .expect("the packs resolve");
    policy.rules.into_iter().map(|r| r.id).collect()
}

#[test]
fn each_manifest_brings_the_rules_of_its_language() {
    let cases = [
        ("pyproject.toml", "no-unsafe-deserialization"),
        ("Package.swift", "no-fatal-error"),
        ("mix.exs", "no-atom-from-input"),
        ("CMakeLists.txt", "no-unbounded-c-string"),
        ("App.csproj", "no-weak-hash"),
        ("Gemfile", "no-permit-all"),
        ("pom.xml", "no-print-stack-trace"),
    ];
    for (manifest, rule) in cases {
        let rules = rules_for(manifest);
        assert!(rules.iter().any(|r| r == rule), "{manifest}: {rules:?}");
    }
    assert!(
        !rules_for("Package.swift")
            .iter()
            .any(|r| r == "no-permit-all")
    );
}

#[test]
fn the_packs_ported_from_typescript_keep_what_they_had_and_what_was_added_has_a_detector() {
    let text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../debuggatha-core/tests/fixtures/knowledge-conformance.json"
    ))
    .unwrap();
    let fixture: serde_json::Value = serde_json::from_str(&text).unwrap();
    let packs = builtin_packs();
    let registry = debuggatha_engine::detect::default_registry();
    let ported = fixture["realPacks"].as_array().unwrap();
    assert_eq!(ported.len(), 30);
    for want in ported {
        let got = packs
            .iter()
            .find(|p| want["id"] == p.id.as_str())
            .unwrap_or_else(|| panic!("{} is not shipped", want["id"]));
        let got = serde_json::to_value(got).unwrap();
        for section in ["rules", "knowledge"] {
            for item in want[section].as_array().unwrap() {
                assert!(
                    got[section].as_array().unwrap().contains(item),
                    "{} lost or changed {}",
                    want["id"],
                    item["id"]
                );
            }
        }
        for rule in got["rules"].as_array().unwrap() {
            if !want["rules"].as_array().unwrap().contains(rule) {
                assert!(
                    registry.has(rule["id"].as_str().unwrap()),
                    "{} has no detector",
                    rule["id"]
                );
            }
        }
        for key in ["id", "version", "kind", "displayName", "dependsOn"] {
            assert_eq!(got[key], want[key], "{} {key}", want["id"]);
        }
    }
}

/// How many rules of the shipped packs each language has a detector for: the number a review of a
/// repository in that language can actually check. Printed with `--nocapture`.
fn checked_rules(language: Language) -> Vec<String> {
    let scope: &[&str] = match language {
        Language::JavaScript | Language::Jsx => &["javascript", "nodejs"],
        Language::TypeScript | Language::Tsx => &["javascript", "typescript", "nodejs"],
        other => &[other.id()],
    };
    let mut rules: Vec<String> = builtin_packs()
        .into_iter()
        .flat_map(|p| p.rules)
        .filter(|r| match &r.applies_to {
            RuleScope::Always => true,
            RuleScope::RequiresLanguage { language } => scope.contains(&language.as_str()),
            _ => false,
        })
        .filter(|r| languages_checked(&r.id).contains(&language))
        .map(|r| r.id)
        .collect();
    rules.sort();
    rules.dedup();
    rules
}

#[test]
fn no_language_is_left_far_behind_the_javascript_family() {
    let count = |l| checked_rules(l).len();
    let javascript = count(Language::TypeScript);
    let others = [
        Language::Rust,
        Language::Go,
        Language::Dart,
        Language::Kotlin,
        Language::Java,
        Language::CSharp,
        Language::Php,
        Language::Python,
        Language::Ruby,
        Language::Elixir,
        Language::Swift,
        Language::Cpp,
    ];
    for language in others {
        println!("{:8} {:3}", language.id(), count(language));
    }
    println!("{:8} {javascript:3}", "typescript");
    let fewest = others.iter().map(|l| count(*l)).min().unwrap();
    assert!(
        fewest * 2 >= javascript,
        "a language has {fewest} checked rules against {javascript} for TypeScript"
    );
}

#[test]
fn a_framework_named_by_the_dependencies_brings_its_rules() {
    let cases = [
        ("requirements.txt", "Django==5.0\n", "django-debug-enabled"),
        ("requirements.txt", "flask>=3\n", "flask-secret-key-in-code"),
        (
            "Gemfile",
            "gem 'rails', '~> 7.1'\n",
            "rails-skip-authentication",
        ),
        (
            "pom.xml",
            "<artifactId>spring-boot-starter-web</artifactId>",
            "spring-csrf-disabled",
        ),
        (
            "mix.exs",
            "{:phoenix, \"~> 1.7\"}",
            "ecto-fragment-interpolation",
        ),
        (
            "App.csproj",
            "<Project Sdk=\"Microsoft.NET.Sdk.Web\"></Project>",
            "aspnet-new-httpclient",
        ),
    ];
    for (manifest, content, rule) in cases {
        let rules = rules_for_content(manifest, content);
        assert!(rules.iter().any(|r| r == rule), "{manifest}: {rules:?}");
    }
    assert!(
        !rules_for_content("requirements.txt", "requests\n")
            .iter()
            .any(|r| r == "django-debug-enabled")
    );
}
