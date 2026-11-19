//! The dependency graph between module boundaries. Two kinds of edge: `declared`, read from each
//! boundary's own manifest, and `import`, found in its files by resolving what they import. An
//! import that resolves to nothing is listed in `unresolved` with its reason, never guessed.

use std::collections::BTreeMap;
use std::path::Path;

use serde::Serialize;

use super::boundaries::{Boundaries, Manifest, ModuleBoundary, manifest_of};
use super::imports::{ImportRef, js_imports};
use super::resolve::{Resolution, Resolver, Unresolved};
use super::rust::{ModuleTrees, RustFile, read_rust};
use super::workspace::{Workspace, is_rust_file};
use crate::analysis::ecosystem::{Dependency, Ecosystems};
use crate::semantic::model_of;
use crate::syntax::{Language, parse};

/// How one module comes to depend on another.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum EdgeKind {
    /// Its manifest names the other.
    Declared,
    /// Its files import the other.
    Import,
}

/// Where a dependency is seen.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Evidence {
    /// The file.
    pub file: String,
    /// What it says.
    pub detail: String,
    /// The line, when there is one.
    pub line: Option<u32>,
}

/// One module depending on another.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DependencyEdge {
    /// The depending module's id.
    pub from: String,
    /// The module depended on.
    pub to: String,
    /// How the dependency is seen.
    pub kind: EdgeKind,
    /// Where it is seen.
    pub evidence: Vec<Evidence>,
}

/// An import the analysis could not follow.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UnresolvedImport {
    /// The file with the import.
    pub file: String,
    /// The line.
    pub line: u32,
    /// What the import names; `None` when it is computed.
    pub specifier: Option<String>,
    /// Why it could not be followed.
    pub reason: &'static str,
}

/// The dependency graph between modules.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct DependencyGraph {
    /// The modules' ids.
    pub nodes: Vec<String>,
    /// The dependencies.
    pub edges: Vec<DependencyEdge>,
    /// The imports that resolved to nothing.
    pub unresolved: Vec<UnresolvedImport>,
    /// Files whose imports could not be read (a fatal parse error): nothing is concluded about them.
    pub unread_files: Vec<String>,
    /// Rust files no `mod` reaches from a crate root (modules declared by a macro such as
    /// `automod::dir!`, fixtures, scripts): the paths they write are not resolved.
    pub unrooted_files: Vec<String>,
}

type Key = (String, String, EdgeKind);

struct Edges(BTreeMap<Key, DependencyEdge>);

impl Edges {
    fn upsert(&mut self, from: &str, to: &str, kind: EdgeKind, evidence: Evidence) {
        if from == to {
            return;
        }
        self.0
            .entry((from.to_owned(), to.to_owned(), kind))
            .or_insert_with(|| DependencyEdge {
                from: from.to_owned(),
                to: to.to_owned(),
                kind,
                evidence: Vec::new(),
            })
            .evidence
            .push(evidence);
    }
}

/// The boundary a file belongs to: the one with the longest directory that contains it. A Rust
/// file `src/a.rs` belongs to the boundary `src/a`, which is the same module.
fn boundary_of<'b>(boundaries: &'b [ModuleBoundary], file: &str) -> Option<&'b str> {
    boundaries
        .iter()
        .filter(|b| {
            b.id == "."
                || file == b.id
                || file.starts_with(&format!("{}/", b.id))
                || file.strip_suffix(".rs") == Some(b.id.as_str())
        })
        .max_by_key(|b| b.id.len())
        .map(|b| b.id.as_str())
}

fn declared_npm(
    ws: &Workspace,
    boundary: &ModuleBoundary,
    by_name: &BTreeMap<&str, &str>,
    edges: &mut Edges,
) {
    let Some(text) = ws.read(&super::workspace::join(&boundary.id, "package.json")) else {
        return;
    };
    let Ok(json) = serde_json::from_str::<serde_json::Value>(text) else {
        return;
    };
    let mut seen = Vec::new();
    for field in ["dependencies", "devDependencies", "peerDependencies"] {
        if let Some(map) = json.get(field).and_then(|v| v.as_object()) {
            for name in map.keys() {
                if !seen.contains(name) {
                    seen.push(name.clone());
                }
            }
        }
    }
    for name in seen {
        if let Some(target) = by_name.get(name.as_str()) {
            edges.upsert(
                &boundary.id,
                target,
                EdgeKind::Declared,
                Evidence {
                    file: "package.json".into(),
                    detail: format!("declares dependency on \"{name}\""),
                    line: None,
                },
            );
        }
    }
}

/// The names a `Cargo.toml` depends on, with the `path` of a path dependency, in the order written.
fn cargo_dependencies(text: &str) -> Vec<(String, Option<String>)> {
    let Ok(doc) = text.parse::<toml::Table>() else {
        return Vec::new();
    };
    let mut tables: Vec<&toml::Table> = Vec::new();
    for key in ["dependencies", "dev-dependencies", "build-dependencies"] {
        if let Some(t) = doc.get(key).and_then(|v| v.as_table()) {
            tables.push(t);
        }
    }
    if let Some(targets) = doc.get("target").and_then(|v| v.as_table()) {
        for target in targets.values().filter_map(|v| v.as_table()) {
            for key in ["dependencies", "dev-dependencies", "build-dependencies"] {
                if let Some(t) = target.get(key).and_then(|v| v.as_table()) {
                    tables.push(t);
                }
            }
        }
    }
    let mut out: Vec<(String, Option<String>)> = Vec::new();
    for table in tables {
        for (key, value) in table {
            let name = value
                .get("package")
                .and_then(|p| p.as_str())
                .unwrap_or(key)
                .to_owned();
            let path = value
                .get("path")
                .and_then(|p| p.as_str())
                .map(str::to_owned);
            if !out.iter().any(|(n, _)| *n == name) {
                out.push((name, path));
            }
        }
    }
    out
}

fn declared_cargo(
    ws: &Workspace,
    boundary: &ModuleBoundary,
    boundaries: &[ModuleBoundary],
    crates: &BTreeMap<String, &str>,
    edges: &mut Edges,
) {
    let Some(text) = ws.read(&super::workspace::join(&boundary.id, "Cargo.toml")) else {
        return;
    };
    for (name, path) in cargo_dependencies(text) {
        let by_path = path
            .map(|p| super::workspace::join(&boundary.id, &p))
            .filter(|dir| boundaries.iter().any(|b| b.id == *dir));
        let target = by_path.or_else(|| {
            crates
                .get(&name.replace('-', "_"))
                .map(|id| (*id).to_owned())
        });
        if let Some(target) = target {
            edges.upsert(
                &boundary.id,
                &target,
                EdgeKind::Declared,
                Evidence {
                    file: "Cargo.toml".into(),
                    detail: format!("declares dependency on \"{name}\""),
                    line: None,
                },
            );
        }
    }
}

/// Rust: every file is read once, then the trees say what each path names.
fn read_rust_files(
    ws: &Workspace,
    boundaries: &[ModuleBoundary],
    unread: &mut Vec<String>,
) -> BTreeMap<String, RustFile> {
    let mut rust_files: BTreeMap<String, RustFile> = BTreeMap::new();
    for boundary in boundaries {
        for file in boundary.files.iter().filter(|f| is_rust_file(f)) {
            match ws.read(file).and_then(read_rust) {
                Some(parsed) => {
                    rust_files.insert(file.clone(), parsed);
                }
                None => unread.push(file.clone()),
            }
        }
    }
    // Files outside every boundary (`src/lib.rs` of a crate split into `src/<dir>`) still declare
    // the modules, so the trees need them.
    for file in ws.files_with(&["rs"]) {
        if !rust_files.contains_key(file)
            && let Some(parsed) = ws.read(file).and_then(read_rust)
        {
            rust_files.insert(file.to_owned(), parsed);
        }
    }
    rust_files
}

/// A package name as another manifest spells it: Python treats `-`, `_` and `.` alike.
fn same_name(ecosystem: &str, a: &str, b: &str) -> bool {
    if ecosystem == "python" {
        let fold = |s: &str| s.to_lowercase().replace(['-', '.'], "_");
        fold(a) == fold(b)
    } else {
        a == b
    }
}

/// Dependencies the manifests of the other ecosystems declare on a module of the repository.
fn declared_ecosystems(
    ecosystems: &Ecosystems<'_>,
    boundaries: &[ModuleBoundary],
    edges: &mut Edges,
) {
    for manifest in ecosystems.manifests() {
        let own = if manifest.dir.is_empty() {
            "."
        } else {
            manifest.dir.as_str()
        };
        let kind = manifest_of(&manifest.ecosystem);
        if !boundaries.iter().any(|b| b.id == own && b.manifest == kind) {
            continue;
        }
        for dependency in ecosystems.dependencies(manifest) {
            let (label, target) = match &dependency {
                Dependency::Name(name) => (
                    name.clone(),
                    boundaries.iter().find(|b| {
                        b.manifest == kind
                            && b.name
                                .as_deref()
                                .is_some_and(|n| same_name(&manifest.ecosystem, n, name))
                    }),
                ),
                Dependency::Path(path) => {
                    let joined = super::workspace::join(&manifest.dir, path);
                    let dir = super::workspace::dirname(&joined);
                    (
                        path.clone(),
                        boundaries
                            .iter()
                            .find(|b| b.manifest == kind && (b.id == joined || b.id == dir)),
                    )
                }
            };
            if let Some(target) = target {
                edges.upsert(
                    own,
                    &target.id,
                    EdgeKind::Declared,
                    Evidence {
                        file: manifest.path.clone(),
                        detail: format!("declares dependency on \"{label}\""),
                        line: None,
                    },
                );
            }
        }
    }
}

/// Builds the graph. A file whose language has no import reader is not read, and the graph says so
/// only through the coverage of its language.
#[must_use]
pub fn build_graph(ws: &Workspace, found: &Boundaries) -> DependencyGraph {
    let boundaries = &found.list;
    let resolver = Resolver::new(ws, boundaries);
    let ecosystems = Ecosystems::new(ws);
    let by_name: BTreeMap<&str, &str> = boundaries
        .iter()
        .filter(|b| b.manifest == Manifest::Npm)
        .filter_map(|b| Some((b.name.as_deref()?, b.id.as_str())))
        .collect();

    let mut edges = Edges(BTreeMap::new());
    let mut unresolved = Vec::new();
    let mut unread = Vec::new();

    let rust_files = read_rust_files(ws, boundaries, &mut unread);
    let trees = ModuleTrees::build(ws, &rust_files);
    let crates: BTreeMap<String, &str> = boundaries
        .iter()
        .filter(|b| b.manifest == Manifest::Cargo)
        .filter_map(|b| Some((b.name.as_deref()?.replace('-', "_"), b.id.as_str())))
        .collect();

    declared_ecosystems(&ecosystems, boundaries, &mut edges);
    for boundary in boundaries {
        declared_npm(ws, boundary, &by_name, &mut edges);
        declared_cargo(ws, boundary, boundaries, &crates, &mut edges);
    }

    for boundary in boundaries {
        for file in &boundary.files {
            let Some(source) = ws.read(file) else {
                continue;
            };
            if is_rust_file(file) {
                if let Some(parsed) = rust_files.get(file) {
                    for r in &parsed.refs {
                        record_rust(
                            &trees,
                            &crates,
                            boundaries,
                            &boundary.id,
                            file,
                            r,
                            &mut edges,
                        );
                    }
                }
                continue;
            }
            if let Some(language) = Language::from_path(Path::new(file))
                && Ecosystems::of(language).is_some()
            {
                let at = (boundaries.as_slice(), boundary.id.as_str(), file.as_str());
                if !record_ecosystem(
                    &ecosystems,
                    language,
                    source,
                    at,
                    &mut edges,
                    &mut unresolved,
                ) {
                    unread.push(file.clone());
                }
                continue;
            }
            let Some(refs) = js_imports(file, source) else {
                unread.push(file.clone());
                continue;
            };
            for r in refs {
                record(
                    &resolver,
                    boundaries,
                    &boundary.id,
                    file,
                    &r,
                    &mut edges,
                    &mut unresolved,
                );
            }
        }
    }

    let unrooted: Vec<String> = boundaries
        .iter()
        .flat_map(|b| b.files.iter())
        .filter(|f| is_rust_file(f) && rust_files.contains_key(*f) && !trees.contains_file(f))
        .cloned()
        .collect();

    DependencyGraph {
        unrooted_files: unrooted,
        nodes: {
            let mut n: Vec<String> = boundaries.iter().map(|b| b.id.clone()).collect();
            n.sort_by(|a, b| debuggatha_core::js::utf16_cmp(a, b));
            n
        },
        edges: edges.0.into_values().collect(),
        unresolved,
        unread_files: unread,
    }
}

fn record_rust(
    trees: &ModuleTrees,
    crates: &BTreeMap<String, &str>,
    boundaries: &[ModuleBoundary],
    own: &str,
    file: &str,
    r: &super::rust::RustRef,
    edges: &mut Edges,
) {
    let target = if trees.is_local(file, r) || r.decl {
        trees
            .resolve(file, r)
            .and_then(|f| boundary_of(boundaries, &f).map(str::to_owned))
    } else {
        r.segments
            .first()
            .and_then(|first| crates.get(first.as_str()))
            .map(|id| (*id).to_owned())
    };
    if let Some(target) = target {
        let path = r.segments.join("::");
        edges.upsert(
            own,
            &target,
            EdgeKind::Import,
            Evidence {
                file: file.to_owned(),
                detail: if r.decl {
                    format!("declares module \"{path}\"")
                } else {
                    format!("imports \"{path}\"")
                },
                line: Some(r.line),
            },
        );
    }
}

/// Reads the imports of a file of an ecosystem the descriptors know, through the semantic model.
/// `false` when the file could not be parsed.
fn record_ecosystem(
    ecosystems: &Ecosystems<'_>,
    language: Language,
    source: &str,
    (boundaries, own, file): (&[ModuleBoundary], &str, &str),
    edges: &mut Edges,
    unresolved: &mut Vec<UnresolvedImport>,
) -> bool {
    let Some(model) = parse(Path::new(file), source).and_then(|syntax| model_of(&syntax, source))
    else {
        return false;
    };
    for import in &model.imports {
        let resolution = ecosystems
            .resolve(file, language, &import.specifier)
            .unwrap_or(Resolution::External);
        Written {
            boundaries,
            own,
            file,
            line: import.line,
            specifier: &import.specifier,
        }
        .record(resolution, edges, unresolved);
    }
    true
}

/// One import statement of a file, ready to be turned into an edge or an unresolved entry.
struct Written<'a> {
    boundaries: &'a [ModuleBoundary],
    own: &'a str,
    file: &'a str,
    line: u32,
    specifier: &'a str,
}

impl Written<'_> {
    fn record(
        &mut self,
        resolution: Resolution,
        edges: &mut Edges,
        unresolved: &mut Vec<UnresolvedImport>,
    ) {
        let target = match resolution {
            Resolution::File(path) => boundary_of(self.boundaries, &path).map(str::to_owned),
            // A manifest at the root has the directory "" and the module id ".".
            Resolution::Package(id) => {
                let id = if id.is_empty() { ".".to_owned() } else { id };
                self.boundaries.iter().any(|b| b.id == id).then_some(id)
            }
            Resolution::External => None,
            Resolution::Unresolved(reason) => {
                unresolved.push(UnresolvedImport {
                    file: self.file.to_owned(),
                    line: self.line,
                    specifier: Some(self.specifier.to_owned()),
                    reason: reason.as_str(),
                });
                None
            }
        };
        if let Some(target) = target {
            edges.upsert(
                self.own,
                &target,
                EdgeKind::Import,
                Evidence {
                    file: self.file.to_owned(),
                    detail: format!("imports \"{}\"", self.specifier),
                    line: Some(self.line),
                },
            );
        }
    }
}

fn record(
    resolver: &Resolver<'_>,
    boundaries: &[ModuleBoundary],
    own: &str,
    file: &str,
    r: &ImportRef,
    edges: &mut Edges,
    unresolved: &mut Vec<UnresolvedImport>,
) {
    let Some(specifier) = &r.specifier else {
        unresolved.push(UnresolvedImport {
            file: file.to_owned(),
            line: r.line,
            specifier: None,
            reason: Unresolved::Computed.as_str(),
        });
        return;
    };
    Written {
        boundaries,
        own,
        file,
        line: r.line,
        specifier,
    }
    .record(resolver.resolve(file, specifier), edges, unresolved);
}
