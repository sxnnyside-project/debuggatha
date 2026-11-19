use indexmap::IndexMap;
use serde_json::Value;

use super::fs::{list_dir, read_text};
use super::stack::StackScan;
use super::types::{Capability, CapabilityKind, CapabilityOrigin, Confidence, Evidence};
use crate::js::{js_get, js_to_string, js_truthy, merge_objects};
use crate::knowledge_system::locale_compare;

use std::path::Path;

/// `name.toLowerCase().replace(/[^a-z0-9]/g, '')`
fn normalize_id(name: &str) -> String {
    name.to_lowercase()
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .collect()
}

fn capability(
    id: &str,
    kind: CapabilityKind,
    confidence: Confidence,
    evidence: Vec<Evidence>,
    origin: CapabilityOrigin,
) -> Capability {
    Capability {
        id: normalize_id(id),
        kind,
        confidence,
        evidence,
        origin,
    }
}

const EXTRA_PACKAGE_JSON_FRAMEWORKS: [(&str, &str); 6] = [
    ("astro", "astro"),
    ("lit", "lit"),
    ("express", "express"),
    ("fastify", "fastify"),
    ("hono", "hono"),
    ("elysia", "elysia"),
];

const TOOLING_CONFIG_FILES: [(&str, &str); 11] = [
    ("biome.json", "biome"),
    ("biome.jsonc", "biome"),
    (".eslintrc.json", "eslint"),
    (".eslintrc.js", "eslint"),
    (".eslintrc.cjs", "eslint"),
    ("eslint.config.js", "eslint"),
    ("eslint.config.mjs", "eslint"),
    ("eslint.config.ts", "eslint"),
    ("vitest.config.ts", "vitest"),
    ("vitest.config.js", "vitest"),
    ("vitest.config.mts", "vitest"),
];

const FEATURE_DIR_NAMES: [&str; 3] = ["features", "modules", "domains"];
const LAYER_DIR_NAMES: [&str; 4] = ["domain", "adapters", "infrastructure", "application"];

fn detect_typescript(root_entries: &[String], package_json: Option<&Value>) -> Option<Capability> {
    let mut evidence = Vec::new();
    let mut confidence = Confidence::Low;
    if root_entries.iter().any(|e| e == "tsconfig.json") {
        evidence.push(Evidence::new(
            "tsconfig.json",
            "tsconfig.json present at repository root",
        ));
        confidence = Confidence::High;
    }
    let deps = merge_objects(
        package_json.and_then(|p| js_get(p, "dependencies")),
        package_json.and_then(|p| js_get(p, "devDependencies")),
    );
    if deps.contains_key("typescript") {
        evidence.push(Evidence::new(
            "package.json",
            "\"typescript\" declared as a dependency",
        ));
        if confidence != Confidence::High {
            confidence = Confidence::Medium;
        }
    }
    if evidence.is_empty() {
        return None;
    }
    Some(capability(
        "typescript",
        CapabilityKind::Language,
        confidence,
        evidence,
        CapabilityOrigin::Manifest,
    ))
}

fn detect_vscode_extension(package_json: Option<&Value>) -> Option<Capability> {
    let vscode = package_json
        .and_then(|p| js_get(p, "engines"))
        .and_then(|e| js_get(e, "vscode"))?;
    if !js_truthy(vscode) {
        return None;
    }
    Some(capability(
        "vscode-extension",
        CapabilityKind::Platform,
        Confidence::High,
        vec![Evidence::new(
            "package.json",
            format!("engines.vscode = \"{}\"", js_to_string(vscode)),
        )],
        CapabilityOrigin::Manifest,
    ))
}

fn detect_php_laravel(root: &Path, root_entries: &[String]) -> Vec<Capability> {
    if !root_entries.iter().any(|e| e == "composer.json") {
        return Vec::new();
    }
    let mut out = vec![capability(
        "php",
        CapabilityKind::Language,
        Confidence::High,
        vec![Evidence::new(
            "composer.json",
            "composer.json present at repository root",
        )],
        CapabilityOrigin::Manifest,
    )];
    // A composer.json that cannot be read or parsed is ignored: PHP is still reported.
    let parsed = read_text(&root.join("composer.json"))
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(&t).ok());
    let requires_laravel = parsed
        .as_ref()
        .and_then(|c| js_get(c, "require"))
        .and_then(Value::as_object)
        .is_some_and(|r| r.contains_key("laravel/framework"));
    if requires_laravel {
        out.push(capability(
            "laravel",
            CapabilityKind::Framework,
            Confidence::High,
            vec![Evidence::new(
                "composer.json",
                "\"laravel/framework\" declared as a dependency",
            )],
            CapabilityOrigin::Manifest,
        ));
    }
    out
}

fn detect_characteristics(root: &Path, root_entries: &[String]) -> Vec<Capability> {
    let has = |name: &str| root_entries.iter().any(|e| e == name);
    let mut out = Vec::new();
    let workspace_dirs: Vec<&str> = ["packages", "apps"]
        .into_iter()
        .filter(|d| has(d))
        .collect();
    if !workspace_dirs.is_empty() {
        let evidence = || -> Vec<Evidence> {
            workspace_dirs
                .iter()
                .map(|d| Evidence::new(*d, format!("\"{d}\" directory present at repository root")))
                .collect()
        };
        out.push(capability(
            "monorepo",
            CapabilityKind::Characteristic,
            Confidence::High,
            evidence(),
            CapabilityOrigin::Convention,
        ));
        out.push(capability(
            "package-based",
            CapabilityKind::Characteristic,
            Confidence::High,
            evidence(),
            CapabilityOrigin::Convention,
        ));
    }
    if has("pnpm-workspace.yaml") || has("turbo.json") {
        let evidence = if has("pnpm-workspace.yaml") {
            Evidence::new("pnpm-workspace.yaml", "pnpm-workspace.yaml present")
        } else {
            Evidence::new("turbo.json", "turbo.json present")
        };
        out.push(capability(
            "workspace",
            CapabilityKind::Characteristic,
            Confidence::High,
            vec![evidence],
            CapabilityOrigin::Convention,
        ));
    }
    let src = root.join("src");
    if src.exists() {
        let entries = list_dir(&src).unwrap_or_default();
        let features: Vec<&String> = entries
            .iter()
            .filter(|e| FEATURE_DIR_NAMES.contains(&e.as_str()))
            .collect();
        let layers: Vec<&String> = entries
            .iter()
            .filter(|e| LAYER_DIR_NAMES.contains(&e.as_str()))
            .collect();
        let listed = |names: &[&String]| -> Vec<Evidence> {
            names
                .iter()
                .map(|e| Evidence::new(format!("src/{e}"), format!("src/{e} directory present")))
                .collect()
        };
        if !features.is_empty() {
            out.push(capability(
                "feature-based",
                CapabilityKind::Characteristic,
                Confidence::Medium,
                listed(&features),
                CapabilityOrigin::Convention,
            ));
        }
        if layers.len() >= 2 {
            out.push(capability(
                "layered",
                CapabilityKind::Characteristic,
                Confidence::Medium,
                listed(&layers),
                CapabilityOrigin::Convention,
            ));
        }
    }
    out
}

/// What a packs rule can match on, derived from the stack profile and a few extra markers.
/// Capabilities that arrive twice merge their evidence; the result is ordered by kind, then id.
#[must_use]
pub fn derive_capabilities(
    root: &Path,
    root_entries: &[String],
    stack: &StackScan,
) -> Vec<Capability> {
    let package_json = stack.package_json.as_ref();
    let profile = &stack.scan.profile;
    let mut all: Vec<Capability> = Vec::new();

    for signal in &profile.languages {
        if signal.value == "JavaScript/TypeScript" {
            all.push(capability(
                "javascript",
                CapabilityKind::Language,
                Confidence::High,
                signal.evidence.clone(),
                CapabilityOrigin::Manifest,
            ));
            all.extend(detect_typescript(root_entries, package_json));
            continue;
        }
        all.push(capability(
            &signal.value,
            CapabilityKind::Language,
            Confidence::High,
            signal.evidence.clone(),
            CapabilityOrigin::Manifest,
        ));
    }
    for signal in &profile.frameworks {
        all.push(capability(
            &signal.value,
            CapabilityKind::Framework,
            Confidence::High,
            signal.evidence.clone(),
            CapabilityOrigin::Manifest,
        ));
    }
    let deps = merge_objects(
        package_json.and_then(|p| js_get(p, "dependencies")),
        package_json.and_then(|p| js_get(p, "devDependencies")),
    );
    for (dependency, id) in EXTRA_PACKAGE_JSON_FRAMEWORKS {
        if deps.contains_key(dependency) {
            all.push(capability(
                id,
                CapabilityKind::Framework,
                Confidence::High,
                vec![Evidence::new(
                    "package.json",
                    format!("dependency \"{dependency}\" declared"),
                )],
                CapabilityOrigin::Manifest,
            ));
        }
    }
    for target in &profile.platform_targets {
        all.push(capability(
            target.as_str(),
            CapabilityKind::Platform,
            Confidence::High,
            vec![Evidence::new(
                "package.json",
                format!(
                    "platform target \"{}\" inferred from detected frameworks",
                    target.as_str()
                ),
            )],
            CapabilityOrigin::Convention,
        ));
    }
    all.extend(detect_vscode_extension(package_json));
    for group in [
        &profile.runtimes,
        &profile.package_managers,
        &profile.build_systems,
    ] {
        for signal in group {
            all.push(capability(
                &signal.value,
                CapabilityKind::Tooling,
                Confidence::High,
                signal.evidence.clone(),
                CapabilityOrigin::Manifest,
            ));
        }
    }
    for (file, id) in TOOLING_CONFIG_FILES {
        if root_entries.iter().any(|e| e == file) {
            all.push(capability(
                id,
                CapabilityKind::Tooling,
                Confidence::High,
                vec![Evidence::new(
                    file,
                    format!("{file} present at repository root"),
                )],
                CapabilityOrigin::Config,
            ));
        }
    }
    all.extend(detect_php_laravel(root, root_entries));
    all.extend(detect_characteristics(root, root_entries));

    merge_and_sort(all)
}

/// Capabilities that arrive twice merge their evidence (the first one keeps its kind, confidence
/// and origin); the result is ordered by kind, then id.
fn merge_and_sort(all: Vec<Capability>) -> Vec<Capability> {
    let mut by_id: IndexMap<String, Capability> = IndexMap::new();
    for cap in all {
        match by_id.get_mut(&cap.id) {
            Some(existing) => existing.evidence.extend(cap.evidence),
            None => {
                by_id.insert(cap.id.clone(), cap);
            }
        }
    }
    let mut merged: Vec<Capability> = by_id.into_values().collect();
    merged.sort_by(|a, b| {
        locale_compare(kind_name(a.kind), kind_name(b.kind))
            .then_with(|| locale_compare(&a.id, &b.id))
    });
    merged
}

fn kind_name(kind: CapabilityKind) -> &'static str {
    match kind {
        CapabilityKind::Language => "language",
        CapabilityKind::Framework => "framework",
        CapabilityKind::Platform => "platform",
        CapabilityKind::Tooling => "tooling",
        CapabilityKind::Characteristic => "characteristic",
    }
}
