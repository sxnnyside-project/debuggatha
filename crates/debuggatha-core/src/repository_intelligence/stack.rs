//! Stack Detection: languages, frameworks, build systems and runtimes from manifests.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde_json::Value;

use super::fs::read_text;
use super::signals::SignalCollector;
use super::types::{Evidence, PlatformTarget, ScanError, ScanResult, StackProfile, WorkspaceType};
use crate::js::{js_get, js_to_string, js_truthy, merge_objects, utf16_cmp};

/// The stack scan and the manifests it read, kept so later scans do not read them again.
pub struct StackScan {
    /// The stack profile and what the scan read.
    pub scan: ScanResult<StackProfile>,
    /// The parsed `package.json`, when the root has one. Shared with the later scanners.
    pub package_json: Option<Value>,
    /// The text of `Cargo.toml`, when there is one.
    pub cargo_toml: Option<String>,
    /// The text of `pubspec.yaml`, when there is one.
    pub pubspec_yaml: Option<String>,
}

const MANIFEST_LANGUAGE: [(&str, &str); 12] = [
    ("Cargo.toml", "Rust"),
    ("pubspec.yaml", "Dart"),
    ("go.mod", "Go"),
    ("Gemfile", "Ruby"),
    ("pyproject.toml", "Python"),
    ("requirements.txt", "Python"),
    ("build.gradle.kts", "Kotlin"),
    ("build.gradle", "Java"),
    ("pom.xml", "Java"),
    ("Package.swift", "Swift"),
    ("mix.exs", "Elixir"),
    ("CMakeLists.txt", "Cpp"),
];

/// Manifests named by extension rather than by name.
const MANIFEST_EXTENSION_LANGUAGE: [(&str, &str); 2] = [(".csproj", "CSharp"), (".sln", "CSharp")];

const PACKAGE_JSON_FRAMEWORKS: [(&str, &str); 7] = [
    ("next", "Next.js"),
    ("react", "React"),
    ("vue", "Vue"),
    ("svelte", "Svelte"),
    ("electron", "Electron"),
    ("@tauri-apps/api", "Tauri"),
    ("@tauri-apps/cli", "Tauri"),
];

const BUILD_SYSTEM_FILES: [(&str, &str); 5] = [
    ("Cargo.toml", "Cargo"),
    ("build.gradle.kts", "Gradle"),
    ("build.gradle", "Gradle"),
    ("pom.xml", "Maven"),
    ("turbo.json", "Turborepo"),
];

const MONOREPO_MARKER_FILES: [(&str, &str); 4] = [
    ("pnpm-workspace.yaml", "pnpm-workspace.yaml present"),
    ("turbo.json", "turbo.json present"),
    ("lerna.json", "lerna.json present"),
    ("nx.json", "nx.json present"),
];

/// A framework named by the dependencies a manifest lists: the manifest, what its text mentions (any
/// case), and the framework it shows.
const FRAMEWORK_MARKERS: &[(&str, &[&str], &str)] = &[
    ("requirements.txt", &["django"], "Django"),
    ("requirements.txt", &["flask"], "Flask"),
    ("pyproject.toml", &["django"], "Django"),
    ("pyproject.toml", &["flask"], "Flask"),
    ("Pipfile", &["django"], "Django"),
    ("Pipfile", &["flask"], "Flask"),
    ("Gemfile", &["'rails'", "\"rails\""], "Rails"),
    ("pom.xml", &["spring-boot", "org.springframework"], "Spring"),
    (
        "build.gradle",
        &["spring-boot", "org.springframework"],
        "Spring",
    ),
    (
        "build.gradle.kts",
        &["spring-boot", "org.springframework"],
        "Spring",
    ),
    ("mix.exs", &[":phoenix"], "Phoenix"),
];

/// A `.csproj` that builds a web application.
const ASPNET_MARKERS: &[&str] = &["Microsoft.NET.Sdk.Web", "Microsoft.AspNetCore"];

/// Everything the root scan accumulates before it becomes a profile.
#[derive(Default)]
struct Collected {
    files_read: Vec<PathBuf>,
    languages: SignalCollector,
    frameworks: SignalCollector,
    build_systems: SignalCollector,
    package_managers: SignalCollector,
    runtimes: SignalCollector,
    manifests: Vec<String>,
    workspace_evidence: Vec<Evidence>,
}

fn present(file: &str) -> Evidence {
    Evidence::new(file, format!("{file} present"))
}

fn at_root(file: &str) -> Evidence {
    Evidence::new(file, format!("{file} present at repository root"))
}

/// Signals that need only the names of the root's entries, not their contents.
fn scan_marker_files(c: &mut Collected, has: &dyn Fn(&str) -> bool) {
    for (manifest, language) in MANIFEST_LANGUAGE {
        if has(manifest) {
            c.manifests.push(manifest.to_owned());
            c.languages.add(language, at_root(manifest));
        }
    }
    for (file, build_system) in BUILD_SYSTEM_FILES {
        if has(file) {
            c.build_systems.add(build_system, at_root(file));
        }
    }
    for (name, configs) in [
        (
            "Vite",
            ["vite.config.ts", "vite.config.js", "vite.config.mjs"].as_slice(),
        ),
        (
            "Webpack",
            ["webpack.config.js", "webpack.config.ts"].as_slice(),
        ),
        ("tsup", ["tsup.config.ts", "tsup.config.js"].as_slice()),
    ] {
        if let Some(file) = configs.iter().find(|f| has(f)) {
            c.build_systems.add(name, present(file));
        }
    }
    if has("bun.lock") || has("bun.lockb") {
        let file = if has("bun.lock") {
            "bun.lock"
        } else {
            "bun.lockb"
        };
        c.package_managers.add("Bun", present(file));
        c.runtimes.add("Bun", present(file));
    }
    for (file, manager) in [
        ("pnpm-lock.yaml", "pnpm"),
        ("package-lock.json", "npm"),
        ("yarn.lock", "Yarn"),
        ("Cargo.lock", "Cargo"),
        ("pubspec.lock", "pub"),
    ] {
        if has(file) {
            c.package_managers.add(manager, present(file));
        }
    }
    if has("bunfig.toml") {
        c.runtimes.add("Bun", present("bunfig.toml"));
    }
    if has("deno.json") || has("deno.jsonc") {
        c.runtimes.add(
            "Deno",
            present(if has("deno.json") {
                "deno.json"
            } else {
                "deno.jsonc"
            }),
        );
    }
    if has(".nvmrc") {
        c.runtimes.add("Node.js", present(".nvmrc"));
    }
    for (file, detail) in MONOREPO_MARKER_FILES {
        if has(file) {
            c.workspace_evidence.push(Evidence::new(file, detail));
        }
    }
}

/// What `package.json` adds: frameworks from its dependencies, workspaces, package manager, engines.
fn scan_package_json(c: &mut Collected, root: &Path) -> Result<Value, ScanError> {
    c.manifests.push("package.json".to_owned());
    c.languages
        .add("JavaScript/TypeScript", at_root("package.json"));
    let path = root.join("package.json");
    c.files_read.push(path.clone());
    let text = read_text(&path)?;
    let invalid = |why: String| ScanError::InvalidJson {
        file: "package.json".to_owned(),
        why,
    };
    let parsed: Value = serde_json::from_str(&text).map_err(|e| invalid(e.to_string()))?;
    if parsed.is_null() {
        // `JSON.parse("null")` succeeds and the next property read throws in the TypeScript engine.
        return Err(invalid("the file contains null".to_owned()));
    }
    let all = merge_objects(
        js_get(&parsed, "dependencies"),
        js_get(&parsed, "devDependencies"),
    );
    for (dependency, framework) in PACKAGE_JSON_FRAMEWORKS {
        if all.contains_key(dependency) {
            c.frameworks.add(
                framework,
                Evidence::new(
                    "package.json",
                    format!("dependency \"{dependency}\" declared"),
                ),
            );
        }
    }
    if js_get(&parsed, "workspaces").is_some_and(js_truthy) {
        c.workspace_evidence.push(Evidence::new(
            "package.json",
            "\"workspaces\" field present",
        ));
    }
    if let Some(Value::String(manager)) = js_get(&parsed, "packageManager") {
        let label = match manager.split('@').next().unwrap_or_default() {
            "bun" => Some("Bun"),
            "pnpm" => Some("pnpm"),
            "yarn" => Some("Yarn"),
            "npm" => Some("npm"),
            _ => None,
        };
        if let Some(label) = label {
            let detail = format!("\"packageManager\": \"{manager}\"");
            c.package_managers
                .add(label, Evidence::new("package.json", detail.clone()));
            if label == "Bun" {
                c.runtimes.add("Bun", Evidence::new("package.json", detail));
            }
        }
    }
    if let Some(node) = js_get(&parsed, "engines")
        .and_then(|e| js_get(e, "node"))
        .filter(|v| js_truthy(v))
    {
        c.runtimes.add(
            "Node.js",
            Evidence::new(
                "package.json",
                format!("engines.node = \"{}\"", js_to_string(node)),
            ),
        );
    }
    Ok(parsed)
}

/// Reads a manifest whose only use here is whether its text names a framework.
fn read_manifest(c: &mut Collected, root: &Path, file: &str) -> Result<String, ScanError> {
    let path = root.join(file);
    c.files_read.push(path.clone());
    read_text(&path)
}

/// The frameworks of the non-JavaScript ecosystems, from what the dependency manifests mention.
/// A manifest that cannot be read shows nothing: it does not fail the scan.
fn scan_framework_manifests(c: &mut Collected, root: &Path, root_entries: &[String]) {
    let mut seen: Vec<(String, String)> = Vec::new();
    let mut check = |c: &mut Collected, file: &str, needles: &[&str], framework: &str| {
        let key = (file.to_owned(), framework.to_owned());
        if seen.contains(&key) {
            return;
        }
        let path = root.join(file);
        let Ok(text) = read_text(&path) else { return };
        c.files_read.push(path);
        let lower = text.to_lowercase();
        if needles.iter().any(|n| lower.contains(&n.to_lowercase())) {
            c.frameworks.add(
                framework,
                Evidence::new(file, "dependency on the framework found"),
            );
        }
        seen.push(key);
    };
    for (file, needles, framework) in FRAMEWORK_MARKERS {
        if root_entries.iter().any(|e| e == file) {
            check(c, file, needles, framework);
        }
    }
    for entry in root_entries.iter().filter(|e| e.ends_with(".csproj")) {
        check(c, entry, ASPNET_MARKERS, "ASP.NET Core");
    }
}

/// Reads the manifests and lockfiles at the repository root.
///
/// A language is only as certain as its marker file: a `Cargo.toml` is reported as Tauri when its
/// text contains `tauri` anywhere, which is the TypeScript behavior carried over (`docs/PARITY.md`).
///
/// # Errors
/// An unreadable manifest, or a `package.json` that is not valid JSON (or is `null`).
pub fn scan_stack(root: &Path, root_entries: &[String]) -> Result<StackScan, ScanError> {
    let has = |name: &str| root_entries.iter().any(|e| e == name);
    let mut c = Collected::default();
    scan_marker_files(&mut c, &has);
    for entry in root_entries {
        if let Some((_, language)) = MANIFEST_EXTENSION_LANGUAGE
            .iter()
            .find(|(extension, _)| entry.ends_with(extension))
        {
            c.manifests.push(entry.clone());
            c.languages.add(language, at_root(entry));
        }
    }

    let package_json = if has("package.json") {
        Some(scan_package_json(&mut c, root)?)
    } else {
        None
    };

    let mut cargo_toml = None;
    if has("Cargo.toml") {
        let text = read_manifest(&mut c, root, "Cargo.toml")?;
        if text.contains("tauri") {
            c.frameworks.add(
                "Tauri",
                Evidence::new("Cargo.toml", "dependency mentioning \"tauri\" found"),
            );
        }
        cargo_toml = Some(text);
    }
    let mut pubspec_yaml = None;
    if has("pubspec.yaml") {
        let text = read_manifest(&mut c, root, "pubspec.yaml")?;
        if text.contains("flutter") {
            c.frameworks.add(
                "Flutter",
                Evidence::new("pubspec.yaml", "dependency mentioning \"flutter\" found"),
            );
        }
        pubspec_yaml = Some(text);
    }

    scan_framework_manifests(&mut c, root, root_entries);

    let workspace_type = if c.workspace_evidence.is_empty() {
        WorkspaceType::SinglePackage
    } else {
        WorkspaceType::Monorepo
    };
    let frameworks = c.frameworks.to_vec();
    let mut targets: BTreeSet<PlatformTarget> = BTreeSet::new();
    for signal in &frameworks {
        match signal.value.as_str() {
            "React" | "Vue" | "Svelte" | "Next.js" => {
                targets.insert(PlatformTarget::Web);
            }
            "Tauri" | "Electron" => {
                targets.insert(PlatformTarget::Desktop);
            }
            "Flutter" => {
                targets.insert(PlatformTarget::Mobile);
            }
            _ => {}
        }
    }
    c.manifests.sort_by(|a, b| utf16_cmp(a, b));
    let profile = StackProfile {
        languages: c.languages.to_vec(),
        frameworks,
        build_systems: c.build_systems.to_vec(),
        package_managers: c.package_managers.to_vec(),
        runtimes: c.runtimes.to_vec(),
        // `Set` order then `.sort()`: desktop, mobile, web is also the derived `Ord` order.
        platform_targets: targets.into_iter().collect(),
        workspace_type,
        workspace_evidence: c.workspace_evidence,
        manifests: c.manifests,
    };
    Ok(StackScan {
        scan: ScanResult {
            profile,
            files_read: c.files_read,
            dirs_listed: Vec::new(),
        },
        package_json,
        cargo_toml,
        pubspec_yaml,
    })
}
