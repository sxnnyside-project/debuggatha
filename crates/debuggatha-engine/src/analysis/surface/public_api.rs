//! The public API of a module: what its entrypoints export, re-exports followed to where each name
//! is declared. A name exported by a file that is not an entrypoint, and that no entrypoint
//! re-exports, is "unexpectedly exposed": an importer can reach it, the module did not declare it.
//!
//! Entrypoints come from the module's `package.json`: `exports` (every subpath, conditions in the
//! order `types`, `import`, `module`, `default`, `require`), else `types`, `module`, `main`; then
//! `src/index` and `index`. A build output (`dist/index.js`) is read as the source it came from.
//! Covered here: JavaScript and TypeScript. A Rust crate's surface (what `pub` reaches from
//! `lib.rs`) is not read; the compiler's `unreachable_pub` lint is the authority.

use std::collections::BTreeSet;

use serde::Serialize;
use serde_json::Value;

use super::exports::{ExportForm, ExportGraph, Facts, Origin};
use crate::analysis::structure::boundaries::{Manifest, ModuleBoundary};
use crate::analysis::structure::resolve::{Resolution, Resolver};
use crate::analysis::structure::workspace::{Workspace, join};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Stability {
    Stable,
    /// The name starts with an underscore: by convention not for consumers.
    Unstable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PublicApiSymbol {
    pub name: String,
    /// The file and line of the statement that exports it from the surface.
    pub file: String,
    pub line: u32,
    pub stability: Stability,
    pub form: ExportForm,
    /// Where it is declared: `(file, name)`, after following re-exports.
    pub definition: Option<(String, String)>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PublicApiSurface {
    pub module_id: String,
    pub entrypoints: Vec<String>,
    /// What each subpath of the package names (`.` and `./v3`), as a source file.
    pub subpaths: Vec<(String, String)>,
    pub exported: Vec<PublicApiSymbol>,
    pub unexpectedly_exposed: Vec<PublicApiSymbol>,
    /// `javascript` (JavaScript and TypeScript) or `rust`.
    pub language: &'static str,
    /// The module has files of that language with a surface to read; a module without any has no surface here.
    pub covered: bool,
    /// What the reading did not do for this module.
    pub limits: Vec<String>,
}

/// A file a tool or a framework loads by its name, whose exports the tool reads (a route's
/// `load`, a config's default, a test's helpers): no importer names them, so "nobody imports it"
/// says nothing about them.
#[must_use]
pub fn loaded_by_convention(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path);
    let lower = name.to_ascii_lowercase();
    name.starts_with('+')
        || [
            ".config.",
            ".setup.",
            ".stories.",
            ".story.",
            ".test.",
            ".spec.",
            ".bench.",
            ".e2e.",
            ".remote.",
            ".tst.",
        ]
        .iter()
        .any(|m| lower.contains(m))
        || lower.starts_with("hooks.")
        || lower.starts_with("middleware.")
        || lower.starts_with("instrumentation.")
        || (path.split('/').any(|seg| seg == "app")
            && [
                "page",
                "route",
                "layout",
                "loading",
                "error",
                "not-found",
                "template",
                "sitemap",
                "robots",
                "manifest",
                "default",
            ]
            .contains(&lower.split('.').next().unwrap_or("")))
        || path.split('/').any(|seg| matches!(seg, "pages" | "routes"))
        || path.split('/').any(|seg| {
            matches!(
                seg,
                "test" | "tests" | "__tests__" | "__mocks__" | "fixtures" | "e2e"
            )
        })
}

fn stability(name: &str) -> Stability {
    if name.starts_with('_') {
        Stability::Unstable
    } else {
        Stability::Stable
    }
}

/// The condition a runtime or compiler would pick first.
const CONDITIONS: &[&str] = &["types", "import", "module", "default", "require"];

/// Every target a manifest value names, the conditions a tool picks first before the others
/// (a repository may add its own, such as `@scope/source`).
fn leaves(value: &Value, out: &mut Vec<String>) {
    match value {
        Value::String(s) => out.push(s.clone()),
        Value::Array(items) => items.iter().for_each(|i| leaves(i, out)),
        Value::Object(map) => {
            for c in CONDITIONS {
                if let Some(v) = map.get(*c) {
                    leaves(v, out);
                }
            }
            for (k, v) in map {
                if !CONDITIONS.contains(&k.as_str()) {
                    leaves(v, out);
                }
            }
        }
        _ => {}
    }
}

/// The source file a manifest target names, among the module's own files.
fn source_of(
    ws: &Workspace,
    resolver: &Resolver<'_>,
    manifest_dir: &str,
    files: &[String],
    target: &str,
) -> Option<String> {
    let from = join(manifest_dir, "package.json");
    let own = |f: &str| files.iter().any(|x| x == f);
    let mut specs = vec![target.to_owned()];
    // `./dist/index.js` is built from `./src/index.ts`.
    let bare = target.trim_start_matches("./");
    if let Some((head, rest)) = bare.split_once('/')
        && matches!(head, "dist" | "build" | "lib" | "out" | "esm" | "cjs")
    {
        specs.push(format!("./src/{rest}"));
        specs.push(format!("./{rest}"));
    }
    specs.into_iter().find_map(|spec| {
        let spec = if spec.starts_with("./") || spec.starts_with("../") {
            spec
        } else {
            format!("./{spec}")
        };
        match resolver.resolve(&from, &spec) {
            Resolution::File(f) if own(&f) && ws.exists(&f) => Some(f),
            _ => None,
        }
    })
}

/// The entrypoints of a module, by subpath, and what the reading could not enumerate.
fn entrypoints(
    ws: &Workspace,
    resolver: &Resolver<'_>,
    manifest_dir: &str,
    files: &[String],
    guess: bool,
    limits: &mut Vec<String>,
) -> Vec<(String, String)> {
    let manifest: Option<Value> = ws
        .read(&join(manifest_dir, "package.json"))
        .and_then(|t| serde_json::from_str(t).ok());
    let mut found: Vec<(String, String)> = Vec::new();
    let first_source = |value: &Value| {
        let mut targets = Vec::new();
        leaves(value, &mut targets);
        targets
            .iter()
            .find_map(|t| source_of(ws, resolver, manifest_dir, files, t))
    };
    if let Some(exports) = manifest.as_ref().and_then(|m| m.get("exports")) {
        let subpaths: Vec<(&str, &Value)> = match exports {
            Value::Object(map) if map.keys().any(|k| k.starts_with('.')) => {
                map.iter().map(|(k, v)| (k.as_str(), v)).collect()
            }
            other => vec![(".", other)],
        };
        for (key, value) in subpaths {
            if key.contains('*') {
                limits.push(format!("export pattern \"{key}\" is not enumerated"));
            } else if let Some(file) = first_source(value) {
                found.push((key.to_owned(), file));
            }
        }
    } else if let Some(m) = &manifest {
        let first = ["types", "typings", "module", "main"]
            .iter()
            .filter_map(|k| m.get(*k).and_then(Value::as_str))
            .find_map(|t| source_of(ws, resolver, manifest_dir, files, t));
        if let Some(f) = first {
            found.push((".".to_owned(), f));
        }
    }
    // With an `exports` field, a package has the entries it lists: no `.` means no main entry.
    // A main entry the manifest declares only as a build output (`dist/pinia.js`) is looked for
    // at the usual source place.
    let main_declared = manifest.as_ref().is_some_and(|m| match m.get("exports") {
        Some(Value::Object(map)) if map.keys().any(|k| k.starts_with('.')) => map.contains_key("."),
        _ => true,
    });
    if guess && main_declared && !found.iter().any(|(k, _)| k == ".") {
        for spec in ["./src/index", "./index"] {
            let from = join(manifest_dir, "package.json");
            if let Resolution::File(f) = resolver.resolve(&from, spec)
                && files.contains(&f)
            {
                found.push((".".to_owned(), f));
                break;
            }
        }
    }
    found
}

/// The public API surface of every module.
#[must_use]
pub fn build_public_api(
    ws: &Workspace,
    boundaries: &[ModuleBoundary],
    facts: &Facts,
    exports: &ExportGraph<'_>,
    resolver: &Resolver<'_>,
) -> Vec<PublicApiSurface> {
    boundaries
        .iter()
        .map(|b| surface(ws, b, facts, exports, resolver))
        .collect()
}

/// The entrypoint files of a module and, for a package of its own, what each subpath names.
fn module_entries(
    ws: &Workspace,
    resolver: &Resolver<'_>,
    boundary: &ModuleBoundary,
    limits: &mut Vec<String>,
) -> (Vec<String>, Vec<(String, String)>) {
    let own_manifest = ws.read(&join(&boundary.id, "package.json")).is_some();
    let subpaths = if own_manifest {
        entrypoints(ws, resolver, &boundary.id, &boundary.files, true, limits)
    } else {
        Vec::new()
    };
    let mut eps: Vec<String> = Vec::new();
    if !own_manifest {
        // A module inside a package (`src/adapter` of a package whose `package.json` is at the
        // root) has the entries that manifest names inside it.
        let mut dir = boundary.id.as_str();
        while let Some((parent, _)) = dir.rsplit_once('/').or(if dir.is_empty() {
            None
        } else {
            Some(("", dir))
        }) {
            dir = parent;
            if ws.read(&join(dir, "package.json")).is_some() {
                let found = entrypoints(ws, resolver, dir, &boundary.files, false, limits);
                eps.extend(found.into_iter().map(|(_, f)| f));
                break;
            }
            if dir.is_empty() {
                break;
            }
        }
        // The usual source place, when no manifest names one.
        if eps.is_empty() {
            for spec in ["./src/index", "./index"] {
                let from = join(&boundary.id, "package.json");
                if let Resolution::File(f) = resolver.resolve(&from, spec)
                    && boundary.files.contains(&f)
                {
                    eps.push(f);
                    break;
                }
            }
        }
    }
    for (_, file) in &subpaths {
        if !eps.contains(file) {
            eps.push(file.clone());
        }
    }
    (eps, subpaths)
}

/// The names the entrypoints export, `export *` followed, and every declaration they make public
/// (a namespace re-export publishes all of its target's names).
fn exported_names(
    eps: &[String],
    facts: &Facts,
    exports: &ExportGraph<'_>,
    resolver: &Resolver<'_>,
) -> (Vec<PublicApiSymbol>, BTreeSet<(String, String)>) {
    let mut exported: Vec<PublicApiSymbol> = Vec::new();
    let symbol = |file: &str, name: &str, line: u32, form: ExportForm| PublicApiSymbol {
        name: name.to_owned(),
        file: file.to_owned(),
        line,
        stability: stability(name),
        form,
        definition: exports.definition(file, name),
    };
    for ep in eps {
        let Some(f) = facts.get(ep) else { continue };
        for e in &f.exports {
            exported.push(symbol(ep, &e.name, e.line, e.form));
        }
        for star in &f.stars {
            if let Resolution::File(target) = resolver.resolve(ep, &star.source) {
                for name in exports
                    .names(&target)
                    .into_iter()
                    .filter(|n| n != "default")
                {
                    if !f.exports.iter().any(|e| e.name == name) {
                        exported.push(symbol(ep, &name, star.line, ExportForm::Star));
                    }
                }
            }
        }
    }
    let mut seen = BTreeSet::new();
    exported.retain(|s| seen.insert((s.file.clone(), s.name.clone())));
    let mut public: BTreeSet<(String, String)> = exported
        .iter()
        .filter_map(|s| s.definition.clone())
        .collect();
    // `export * as ns from './b'` publishes every name of `./b` as a member of `ns`.
    for ep in eps {
        let Some(f) = facts.get(ep) else { continue };
        for e in &f.exports {
            if let Origin::From { source, imported } = &e.origin
                && imported == "*"
                && let Resolution::File(target) = resolver.resolve(ep, source)
            {
                for name in exports.names(&target) {
                    public.extend(exports.definition(&target, &name));
                }
            }
        }
    }
    (exported, public)
}

fn surface(
    ws: &Workspace,
    boundary: &ModuleBoundary,
    facts: &Facts,
    exports: &ExportGraph<'_>,
    resolver: &Resolver<'_>,
) -> PublicApiSurface {
    if boundary.manifest == Manifest::Cargo
        && let Some(rust) = rust_surface_of(ws, boundary)
    {
        return rust;
    }
    let covered = boundary.files.iter().any(|f| facts.contains_key(f));
    let mut limits = Vec::new();
    let (eps, subpaths) = if covered {
        module_entries(ws, resolver, boundary, &mut limits)
    } else {
        (Vec::new(), Vec::new())
    };
    let (exported, public) = exported_names(&eps, facts, exports, resolver);

    let mut exposed = Vec::new();
    for file in &boundary.files {
        if eps.contains(file) || loaded_by_convention(file) {
            continue;
        }
        let Some(f) = facts.get(file) else { continue };
        for e in &f.exports {
            let definition = exports.definition(file, &e.name);
            let reexported = definition.as_ref().is_some_and(|d| public.contains(d));
            // A re-export of another file's name makes that file's name public, not this one's.
            let forwarded = matches!(e.origin, Origin::From { .. });
            // A default export is how a tool loads a file (config, route, page): no importer
            // names it.
            if !reexported && !forwarded && e.name != "default" {
                exposed.push(PublicApiSymbol {
                    name: e.name.clone(),
                    file: file.clone(),
                    line: e.line,
                    stability: stability(&e.name),
                    form: e.form,
                    definition,
                });
            }
        }
    }
    PublicApiSurface {
        module_id: boundary.id.clone(),
        entrypoints: eps,
        subpaths,
        language: "javascript",
        exported,
        unexpectedly_exposed: exposed,
        covered,
        limits,
    }
}

/// The surface of a Rust library crate, when the boundary is one.
fn rust_surface_of(ws: &Workspace, boundary: &ModuleBoundary) -> Option<PublicApiSurface> {
    let manifest = ws.read(&join(&boundary.id, "Cargo.toml"))?;
    let doc: toml::Table = manifest.parse().ok()?;
    let lib = doc
        .get("lib")
        .and_then(|l| l.get("path"))
        .and_then(toml::Value::as_str)
        .map_or_else(|| "src/lib.rs".to_owned(), str::to_owned);
    let root = join(&boundary.id, &lib);
    let (exported, exposed) = super::rust_api::rust_surface(ws, &root)?;
    Some(PublicApiSurface {
        module_id: boundary.id.clone(),
        entrypoints: vec![root.clone()],
        subpaths: vec![(".".to_owned(), root)],
        language: "rust",
        exported,
        unexpectedly_exposed: exposed,
        covered: true,
        limits: vec![
            "the associated items of a type (methods, fields, variants) and impl blocks are not listed"
                .to_owned(),
            "`cfg` is not evaluated: every branch counts".to_owned(),
        ],
    })
}
