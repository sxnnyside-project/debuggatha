//! Module boundaries: the units the rest of the analysis talks about. Whatever the repository
//! already declares wins over a guess: a monorepo's `packages/*` and
//! `apps/*`, and a Cargo workspace's members. Only a repository with neither falls back to the
//! directory names under `src/`. A directory that matches no known vocabulary is still a boundary
//! (`domain`), never dropped.

use serde::Serialize;

use super::ecosystem::Ecosystems;
use super::workspace::{ANALYZED_EXTENSIONS, Workspace, join};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ModuleKind {
    Package,
    Workspace,
    Feature,
    Domain,
    Adapter,
    Layer,
}

/// What manifest backs a boundary: it decides how other code names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Manifest {
    /// `package.json`: other code imports it by `name`.
    Npm,
    /// `Cargo.toml`: other code writes `name_with_underscores::`.
    Cargo,
    /// `go.mod`.
    Go,
    /// `pyproject.toml`.
    Python,
    /// `pubspec.yaml`.
    Pub,
    /// `composer.json`.
    Composer,
    /// `mix.exs`.
    Mix,
    /// `pom.xml`.
    Maven,
    /// `build.gradle.kts`.
    Gradle,
    /// A `.csproj`.
    Dotnet,
    /// A `Gemfile`.
    Bundler,
    /// `Package.swift`.
    SwiftPm,
    None,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ModuleBoundary {
    /// The directory, relative to the root: the id the graph uses.
    pub id: String,
    pub kind: ModuleKind,
    /// The declared package name, if a manifest backs the boundary.
    pub name: Option<String>,
    pub manifest: Manifest,
    /// The analyzed files it owns, sorted.
    pub files: Vec<String>,
}

const WORKSPACE_GROUPS: &[(&str, ModuleKind)] = &[
    ("packages", ModuleKind::Package),
    ("apps", ModuleKind::Workspace),
];

const FEATURE_LIKE: &[&str] = &["features", "modules", "services", "components"];
const ADAPTER_LIKE: &[&str] = &["adapters", "infrastructure", "infra", "gateways", "clients"];
const LAYER_LIKE: &[&str] = &["layers", "core", "application", "presentation", "ui"];

fn kind_for_dir_name(name: &str) -> ModuleKind {
    if ADAPTER_LIKE.contains(&name) {
        ModuleKind::Adapter
    } else if LAYER_LIKE.contains(&name) {
        ModuleKind::Layer
    } else if FEATURE_LIKE.contains(&name) {
        ModuleKind::Feature
    } else {
        ModuleKind::Domain
    }
}

/// Directories the `src/` fallback never makes a boundary of (`scan_scope` ignores them too).
const IGNORED_DIRS: &[&str] = &[
    "node_modules",
    "vendor",
    "bower_components",
    "jspm_packages",
    "Pods",
    "Carthage",
    "target",
    "build",
    "dist",
    "out",
    "coverage",
    "__pycache__",
    "site-packages",
    "venv",
    "obj",
    ".gradle",
    ".dart_tool",
    ".next",
    ".nuxt",
    ".svelte-kit",
    ".turbo",
    ".venv",
];

pub(super) fn manifest_of(ecosystem: &str) -> Manifest {
    match ecosystem {
        "go" => Manifest::Go,
        "python" => Manifest::Python,
        "dart" => Manifest::Pub,
        "php" => Manifest::Composer,
        "elixir" => Manifest::Mix,
        "java" => Manifest::Maven,
        "kotlin" => Manifest::Gradle,
        "csharp" => Manifest::Dotnet,
        "ruby" => Manifest::Bundler,
        "swift" => Manifest::SwiftPm,
        _ => Manifest::None,
    }
}

/// The result: boundaries plus what the inference could not do.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Boundaries {
    pub list: Vec<ModuleBoundary>,
    /// Constructs the inference does not read, stated so a report can say so.
    pub limits: Vec<String>,
}

fn package_name(ws: &Workspace, dir: &str) -> Option<String> {
    let text = ws.read(&join(dir, "package.json"))?;
    let json: serde_json::Value = serde_json::from_str(text).ok()?;
    json.get("name")?.as_str().map(str::to_owned)
}

fn cargo_name(ws: &Workspace, dir: &str) -> Option<String> {
    let text = ws.read(&join(dir, "Cargo.toml"))?;
    let doc: toml::Table = text.parse().ok()?;
    doc.get("package")?.get("name")?.as_str().map(str::to_owned)
}

fn files_under(ws: &Workspace, dir: &str) -> Vec<String> {
    if dir == "." {
        return ws
            .files_with(ANALYZED_EXTENSIONS)
            .into_iter()
            .map(str::to_owned)
            .collect();
    }
    let prefix = format!("{dir}/");
    // `src/a.rs` and the directory `src/a/` are one Rust module: the file belongs to the boundary.
    let own_module = format!("{dir}.rs");
    ws.files_with(ANALYZED_EXTENSIONS)
        .into_iter()
        .filter(|f| f.starts_with(&prefix) || *f == own_module)
        .map(str::to_owned)
        .collect()
}

/// The members of the Cargo workspace at the root, as directories. `dir/*` and plain paths are
/// read; any other glob is a stated limit.
fn cargo_members(ws: &Workspace, limits: &mut Vec<String>) -> Vec<String> {
    let Some(text) = ws.read("Cargo.toml") else {
        return Vec::new();
    };
    let Ok(doc) = text.parse::<toml::Table>() else {
        return Vec::new();
    };
    let strings = |key: &str| -> Vec<String> {
        doc.get("workspace")
            .and_then(|w| w.get(key))
            .and_then(|m| m.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default()
    };
    let excluded = strings("exclude");
    let mut out = Vec::new();
    // A root manifest that is also a package (`[package]` beside `[workspace]`) is a crate too.
    if doc.get("package").is_some() && doc.get("workspace").is_some() {
        out.push(".".to_owned());
    }
    for member in strings("members") {
        let member = member.trim_end_matches('/').to_owned();
        let dirs = if let Some(parent) = member.strip_suffix("/*") {
            if parent.contains(['*', '?', '[']) {
                limits.push(format!(
                    "Cargo workspace member glob \"{member}\" is not read"
                ));
                continue;
            }
            ws.child_dirs(parent)
                .into_iter()
                .map(|c| format!("{parent}/{c}"))
                .collect()
        } else if member.contains(['*', '?', '[']) {
            limits.push(format!(
                "Cargo workspace member glob \"{member}\" is not read"
            ));
            continue;
        } else {
            vec![super::workspace::normalize(&member)]
        };
        for dir in dirs {
            if ws.exists(&join(&dir, "Cargo.toml")) && !excluded.contains(&dir) {
                out.push(dir);
            }
        }
    }
    out
}

fn add_boundary(ws: &Workspace, list: &mut Vec<ModuleBoundary>, dir: String, kind: ModuleKind) {
    if list.iter().any(|b| b.id == dir) {
        return;
    }
    let npm = package_name(ws, &dir);
    let cargo = cargo_name(ws, &dir);
    let (name, manifest) = match (npm, cargo) {
        (Some(n), _) => (Some(n), Manifest::Npm),
        (None, Some(c)) => (Some(c), Manifest::Cargo),
        (None, None) => (None, Manifest::None),
    };
    let files = files_under(ws, &dir);
    list.push(ModuleBoundary {
        id: dir,
        kind,
        name,
        manifest,
        files,
    });
}

/// A manifest of another ecosystem that has sources of its language beside or below it: its
/// directory, its ecosystem and its name. `root` picks the manifests at the repository root.
fn ecosystem_manifests(
    ws: &Workspace,
    found: &Ecosystems<'_>,
    root: bool,
) -> Vec<(String, String, Option<String>)> {
    found
        .manifests()
        .iter()
        .filter(|m| m.dir.is_empty() == root)
        .filter(|m| {
            let Some((_, d)) = super::ecosystem::descriptors()
                .iter()
                .find(|(id, _)| *id == m.ecosystem)
            else {
                return false;
            };
            let under = if root {
                String::new()
            } else {
                format!("{}/", m.dir)
            };
            let extensions: Vec<&str> = d.extensions.iter().map(String::as_str).collect();
            ws.files_with(&extensions)
                .iter()
                .any(|f| f.starts_with(&under))
        })
        .map(|m| {
            let dir = if m.dir.is_empty() { "." } else { &m.dir };
            (dir.to_owned(), m.ecosystem.clone(), m.name.clone())
        })
        .collect()
}

fn add_ecosystem(
    ws: &Workspace,
    list: &mut Vec<ModuleBoundary>,
    (dir, ecosystem, name): (String, String, Option<String>),
) {
    add_boundary(ws, list, dir.clone(), ModuleKind::Package);
    if let Some(b) = list.iter_mut().find(|b| b.id == dir) {
        b.manifest = manifest_of(&ecosystem);
        b.name = name;
    }
}

/// The directories under a single manifest that hold sources of its language, by the descriptor's
/// `module_roots`: `internal/*`, `pkg/*`, then the top-level directories.
fn internal_modules(ws: &Workspace, ecosystem: &str) -> Vec<String> {
    let Some((_, d)) = super::ecosystem::descriptors()
        .iter()
        .find(|(id, _)| id == ecosystem)
    else {
        return Vec::new();
    };
    let extensions: Vec<&str> = d.extensions.iter().map(String::as_str).collect();
    let sources = ws.files_with(&extensions);
    let named_roots: Vec<&str> = d
        .module_roots
        .iter()
        .map(String::as_str)
        .filter(|r| !r.is_empty())
        .collect();
    let mut out = Vec::new();
    for root in &d.module_roots {
        for child in ws.child_dirs(root) {
            let dir = join(root, &child);
            let hidden = child.starts_with(['.', '_']) || IGNORED_DIRS.contains(&child.as_str());
            let is_root = root.is_empty()
                && named_roots
                    .iter()
                    .any(|r| r.split('/').next() == Some(child.as_str()));
            let prefix = format!("{dir}/");
            if !hidden
                && !is_root
                && !out.contains(&dir)
                && sources.iter().any(|f| f.starts_with(&prefix))
            {
                out.push(dir);
            }
        }
    }
    out
}

/// A directory with its own manifest owns its files, not the manifest directory above it.
fn give_nested_files_to_their_owner(list: &mut [ModuleBoundary]) {
    let declared = |b: &ModuleBoundary| {
        !matches!(b.manifest, Manifest::Npm | Manifest::Cargo | Manifest::None)
    };
    let nested: Vec<String> = list
        .iter()
        .filter(|b| declared(b))
        .map(|b| b.id.clone())
        .collect();
    for b in list.iter_mut().filter(|b| declared(b)) {
        let own = b.id.clone();
        b.files.retain(|f| {
            !nested
                .iter()
                .any(|o| *o != own && f.starts_with(&format!("{o}/")))
        });
    }
}

/// The root crate owns what no other boundary does.
fn leave_the_rest_to_the_root(list: &mut [ModuleBoundary]) {
    let others: Vec<String> = list
        .iter()
        .filter(|b| b.id != ".")
        .map(|b| b.id.clone())
        .collect();
    if let Some(root) = list.iter_mut().find(|b| b.id == ".") {
        root.files.retain(|f| {
            !others.iter().any(|o| {
                f.starts_with(&format!("{o}/")) || f.strip_suffix(".rs") == Some(o.as_str())
            })
        });
    }
}

/// Infers the boundaries of a repository.
#[must_use]
pub fn infer_boundaries(ws: &Workspace) -> Boundaries {
    let mut result = Boundaries::default();
    for (group, kind) in WORKSPACE_GROUPS {
        for entry in ws.child_dirs(group) {
            add_boundary(ws, &mut result.list, format!("{group}/{entry}"), *kind);
        }
    }
    for dir in cargo_members(ws, &mut result.limits) {
        add_boundary(ws, &mut result.list, dir, ModuleKind::Package);
    }
    let found = Ecosystems::new(ws);
    for manifest in ecosystem_manifests(ws, &found, false) {
        add_ecosystem(ws, &mut result.list, manifest);
    }
    if result.list.is_empty() {
        for entry in ws.child_dirs("src") {
            if IGNORED_DIRS.contains(&entry.as_str()) || entry.starts_with('.') {
                continue;
            }
            let kind = kind_for_dir_name(&entry);
            add_boundary(ws, &mut result.list, format!("src/{entry}"), kind);
        }
    }
    if result.list.is_empty()
        && let Some(manifest) = ecosystem_manifests(ws, &found, true).into_iter().next()
    {
        let ecosystem = manifest.1.clone();
        add_ecosystem(ws, &mut result.list, manifest);
        for dir in internal_modules(ws, &ecosystem) {
            let kind = kind_for_dir_name(dir.rsplit('/').next().unwrap_or(&dir));
            add_boundary(ws, &mut result.list, dir.clone(), kind);
            if let Some(b) = result.list.iter_mut().find(|b| b.id == dir) {
                b.manifest = manifest_of(&ecosystem);
            }
        }
    }
    give_nested_files_to_their_owner(&mut result.list);
    leave_the_rest_to_the_root(&mut result.list);
    result
        .list
        .sort_by(|a, b| debuggatha_core::js::utf16_cmp(&a.id, &b.id));
    result
}
