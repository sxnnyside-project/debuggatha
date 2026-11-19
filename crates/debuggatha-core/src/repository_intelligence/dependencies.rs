//! Dependency Context: lockfiles, declared versions and runtime constraints.

use indexmap::IndexMap;
use regress::Regex;
use serde_json::Value;

use super::stack::StackScan;
use super::types::{DependencyProfile, Evidence, ScanResult};
use crate::js::{js_get, js_truthy, merge_objects};

/// The contract says versions are strings. A manifest that gives a number or a boolean is read as
/// its text; an object or an array is not a version and is left out. (The TypeScript engine copies
/// whatever it finds, which breaks its own type; see `docs/PARITY.md`.)
fn version_text(value: &Value) -> Option<String> {
    match value {
        Value::String(s) => Some(s.clone()),
        Value::Number(_) | Value::Bool(_) => Some(crate::js::js_to_string(value)),
        _ => None,
    }
}

const LOCKFILES: [&str; 7] = [
    "bun.lock",
    "bun.lockb",
    "pnpm-lock.yaml",
    "package-lock.json",
    "yarn.lock",
    "Cargo.lock",
    "pubspec.lock",
];

const TRACKED_PACKAGES: [&str; 7] = [
    "react",
    "next",
    "vue",
    "svelte",
    "electron",
    "@tauri-apps/api",
    "@tauri-apps/cli",
];

/// The first `key = "value"` line of a TOML file, found with a regular expression. It does not
/// parse TOML, so it also matches inside any table (`docs/PARITY.md`).
fn extract_toml_string_value(text: &str, key: &str) -> Option<String> {
    let pattern = format!("^\\s*{key}\\s*=\\s*\"([^\"]+)\"");
    let regex = Regex::with_flags(&pattern, "m").ok()?;
    let found = regex.find(text)?;
    found.group(1).map(|range| text[range].to_owned())
}

/// Read lockfiles, declared versions and runtime constraints of the repository.
#[must_use]
pub fn scan_dependencies(
    root_entries: &[String],
    stack: &StackScan,
) -> ScanResult<DependencyProfile> {
    let mut lockfiles = Vec::new();
    for file in LOCKFILES {
        if root_entries.iter().any(|e| e == file) {
            lockfiles.push(Evidence::new(file, format!("{file} present")));
        }
    }
    let mut declared_versions: IndexMap<String, String> = IndexMap::new();
    let mut runtime_constraints: IndexMap<String, String> = IndexMap::new();

    if let Some(package_json) = &stack.package_json {
        let all = merge_objects(
            js_get(package_json, "dependencies"),
            js_get(package_json, "devDependencies"),
        );
        for name in TRACKED_PACKAGES {
            if let Some(version) = all.get(name).filter(|v| js_truthy(v))
                && let Some(text) = version_text(version)
            {
                declared_versions.insert(name.to_owned(), text);
            }
        }
        if let Some(node) = js_get(package_json, "engines")
            .and_then(|e| js_get(e, "node"))
            .filter(|v| js_truthy(v))
            && let Some(text) = version_text(node)
        {
            runtime_constraints.insert("node".to_owned(), text);
        }
        if let Some(Value::String(manager)) = js_get(package_json, "packageManager") {
            runtime_constraints.insert("packageManager".to_owned(), manager.clone());
        }
    }
    if let Some(cargo) = &stack.cargo_toml {
        if let Some(version) = extract_toml_string_value(cargo, "tauri") {
            declared_versions.insert("tauri".to_owned(), version);
        }
        if let Some(version) = extract_toml_string_value(cargo, "rust-version") {
            runtime_constraints.insert("rust".to_owned(), version);
        }
    }
    ScanResult {
        profile: DependencyProfile {
            lockfiles,
            declared_versions,
            runtime_constraints,
        },
        files_read: Vec::new(),
        dirs_listed: Vec::new(),
    }
}
