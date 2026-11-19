//! Dead code in JavaScript and TypeScript, found from what files declare and import by name, not
//! from a word that happens to appear somewhere else. Three kinds, none certain:
//!
//! - `unreferenced`: a top-level declaration nothing in its own file reads and that is not
//!   exported (the scopes decide, so a local of the same name does not count as a use).
//! - `exported-but-unused`: a name the module's entrypoint publishes that no file of the
//!   repository imports.
//! - `probably-dead`: a name exported by a file that is not an entrypoint, imported by no file and
//!   read by none in its own.
//!
//! An import that cannot be narrowed to names (`import()`, `require()`, a namespace that is
//! passed on, a deep import of a workspace package) counts as a use of every name it could reach.
//! Not covered: Rust (the compiler's `dead_code` lint is the authority), template-only uses in a
//! single-file component's own declarations, and any consumer outside the repository.

use std::collections::{BTreeMap, BTreeSet};

use debuggatha_core::js::utf16_cmp;
use serde::Serialize;

use super::boundaries::ModuleBoundary;
use super::exports::{ExportGraph, Facts, ImportFact};
use super::public_api::{PublicApiSurface, PublicApiSymbol};
use super::resolve::{Resolution, Resolver, Unresolved};
use super::workspace::{Workspace, join};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum DeadCodeKind {
    Unreferenced,
    ExportedButUnused,
    ProbablyDead,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Confidence {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DeadCodeFinding {
    pub file: String,
    pub symbol: String,
    pub line: u32,
    pub kind: DeadCodeKind,
    pub confidence: Confidence,
    pub rationale: String,
}

type Key = (String, String);

/// Every `(file, name)` some file reads, by the declaration the name leads to.
struct Uses<'a> {
    exports: &'a ExportGraph<'a>,
    used: BTreeSet<Key>,
}

impl Uses<'_> {
    fn name(&mut self, file: &str, name: &str) {
        let key = self
            .exports
            .definition(file, name)
            .unwrap_or_else(|| (file.to_owned(), name.to_owned()));
        self.used.insert(key);
    }

    fn all(&mut self, file: &str) {
        for n in self.exports.names(file) {
            self.name(file, &n);
        }
    }
}

fn publishable(ws: &Workspace, boundary: &ModuleBoundary) -> bool {
    ws.read(&join(&boundary.id, "package.json"))
        .and_then(|t| serde_json::from_str::<serde_json::Value>(t).ok())
        .is_some_and(|m| m.get("name").is_some() && m.get("private") != Some(&true.into()))
}

/// What the pass reads: the files, how to resolve an import, the modules by id.
struct Ctx<'a> {
    ws: &'a Workspace,
    facts: &'a Facts,
    resolver: &'a Resolver<'a>,
    apis: BTreeMap<&'a str, &'a PublicApiSurface>,
    boundaries: BTreeMap<&'a str, &'a ModuleBoundary>,
}

impl Ctx<'_> {
    /// Every file of a package: what a path into it that nothing names could reach.
    fn mark_whole_package(&self, uses: &mut Uses<'_>, id: &str) {
        if let Some(b) = self.boundaries.get(id) {
            for file in &b.files {
                uses.all(file);
            }
        }
    }
}

/// The names `import` takes from `target`.
fn mark_import(uses: &mut Uses<'_>, target: &str, import: &ImportFact) {
    for n in &import.names {
        uses.name(target, n);
    }
    if let Some(ns) = &import.namespace {
        if ns.escapes {
            uses.all(target);
        }
        for m in &ns.members {
            uses.name(target, m);
        }
    }
}

/// Every `(file, name)` some file reads, and the names read through an alias nobody resolves.
fn collect_uses<'a>(ctx: &Ctx<'a>, exports: &'a ExportGraph<'a>) -> (Uses<'a>, BTreeSet<&'a str>) {
    let mut uses = Uses {
        exports,
        used: BTreeSet::new(),
    };
    // Names imported through an alias the analysis does not read (`#app/x`, `@/x`): they reach
    // some file, so no declaration of that name is called dead.
    let mut loose: BTreeSet<&str> = BTreeSet::new();
    for (file, f) in ctx.facts {
        for import in &f.imports {
            match ctx.resolver.resolve(file, &import.source) {
                Resolution::File(target) => mark_import(&mut uses, &target, import),
                Resolution::Unresolved(
                    Unresolved::UnknownAlias | Unresolved::AliasTargetNotFound,
                ) => {
                    loose.extend(import.names.iter().map(String::as_str));
                    if let Some(ns) = &import.namespace {
                        loose.extend(ns.members.iter().map(String::as_str));
                    }
                }
                // A stylesheet, an image or JSON of a package carries no name.
                Resolution::Package(_) if !is_script(&import.source) => {}
                Resolution::Package(id) => {
                    match package_file(
                        &id,
                        &import.source,
                        &ctx.apis,
                        &ctx.boundaries,
                        ctx.resolver,
                    ) {
                        Some(target) => mark_import(&mut uses, &target, import),
                        // A path into the package that no manifest entry or file names.
                        None => ctx.mark_whole_package(&mut uses, &id),
                    }
                }
                _ => {}
            }
        }
    }
    // `import()`, `require()` and `import('x').T` name no member: all of the target is used.
    for file in ctx.facts.keys() {
        let Some(text) = ctx.ws.read(file) else {
            continue;
        };
        for r in super::imports::js_imports(file, text).unwrap_or_default() {
            use super::imports::ImportKind::{Dynamic, Require, TypeQuery};
            let Some(specifier) = r
                .specifier
                .filter(|_| matches!(r.kind, Dynamic | Require | TypeQuery))
            else {
                continue;
            };
            match ctx.resolver.resolve(file, &specifier) {
                Resolution::File(t) => uses.all(&t),
                Resolution::Package(id) => {
                    match package_file(&id, &specifier, &ctx.apis, &ctx.boundaries, ctx.resolver) {
                        Some(t) => uses.all(&t),
                        None => ctx.mark_whole_package(&mut uses, &id),
                    }
                }
                _ => {}
            }
        }
    }
    (uses, loose)
}

fn key_of(s: &PublicApiSymbol) -> Key {
    s.definition
        .clone()
        .unwrap_or_else(|| (s.file.clone(), s.name.clone()))
}

/// Names a module publishes that no file imports.
fn unused_public(
    api: &PublicApiSurface,
    library: bool,
    dynamic: usize,
    used: &dyn Fn(&PublicApiSymbol) -> bool,
    out: &mut Vec<DeadCodeFinding>,
) {
    for s in api.exported.iter().filter(|s| !used(s)) {
        let (confidence, caveat) = if dynamic > 0 {
            (
                Confidence::Low,
                format!(" The module also loads {dynamic} computed path(s), which could reach it."),
            )
        } else if library {
            (
                Confidence::Low,
                " The module is publishable: consumers outside this repository are not visible."
                    .to_owned(),
            )
        } else {
            (Confidence::Medium, String::new())
        };
        out.push(DeadCodeFinding {
            file: s.file.clone(),
            symbol: s.name.clone(),
            line: s.line,
            kind: DeadCodeKind::ExportedButUnused,
            confidence,
            rationale: format!(
                "\"{}\" is exported from {}'s public entrypoint and no file in the repository imports it.{caveat}",
                s.name, api.module_id
            ),
        });
    }
}

/// Names exported by a file that is not an entrypoint, imported by none and read by none.
fn unused_exposed(
    api: &PublicApiSurface,
    facts: &Facts,
    used: &dyn Fn(&PublicApiSymbol) -> bool,
    out: &mut Vec<DeadCodeFinding>,
) {
    for s in api.unexpectedly_exposed.iter().filter(|s| !used(s)) {
        // Read in its own file, the name is not dead; that the `export` reaches nobody is for the
        // public API report (`unexpectedly_exposed`), not for this one.
        let read_here = facts
            .get(&s.file)
            .and_then(|f| {
                f.exports
                    .iter()
                    .find(|e| e.name == s.name && e.line == s.line)
            })
            .is_some_and(|e| e.used_here);
        if read_here {
            continue;
        }
        out.push(DeadCodeFinding {
            file: s.file.clone(),
            symbol: s.name.clone(),
            line: s.line,
            kind: DeadCodeKind::ProbablyDead,
            confidence: Confidence::Low,
            rationale: format!(
                "\"{}\" is exported from {}, which is not an entrypoint of {}, and nothing imports it or reads it in its own file.",
                s.name, s.file, api.module_id
            ),
        });
    }
}

#[must_use]
pub fn detect_dead_code(
    ws: &Workspace,
    boundaries: &[ModuleBoundary],
    facts: &Facts,
    exports: &ExportGraph<'_>,
    resolver: &Resolver<'_>,
    apis: &[PublicApiSurface],
) -> Vec<DeadCodeFinding> {
    let ctx = Ctx {
        ws,
        facts,
        resolver,
        apis: apis.iter().map(|a| (a.module_id.as_str(), a)).collect(),
        boundaries: boundaries.iter().map(|b| (b.id.as_str(), b)).collect(),
    };
    let (reads, loose) = collect_uses(&ctx, exports);
    let used =
        |s: &PublicApiSymbol| reads.used.contains(&key_of(s)) || loose.contains(s.name.as_str());

    let computed = computed_per_module(ws, boundaries, facts);
    let mut out = Vec::new();
    for api in apis.iter().filter(|a| a.language == "javascript") {
        let Some(boundary) = ctx.boundaries.get(api.module_id.as_str()) else {
            continue;
        };
        let dynamic = computed.get(api.module_id.as_str()).copied().unwrap_or(0);
        unused_public(api, publishable(ws, boundary), dynamic, &used, &mut out);
        unused_exposed(api, facts, &used, &mut out);
    }
    for (file, f) in facts {
        for u in &f.unreferenced {
            out.push(DeadCodeFinding {
                file: file.clone(),
                symbol: u.name.clone(),
                line: u.line,
                kind: DeadCodeKind::Unreferenced,
                confidence: if matches!(u.kind, "variable" | "enum") {
                    Confidence::Medium
                } else {
                    Confidence::High
                },
                rationale: format!(
                    "The {} \"{}\" is declared in {file} and nothing in the file reads it; it is not exported.",
                    u.kind, u.name
                ),
            });
        }
    }
    out.sort_by(|a, b| {
        utf16_cmp(&a.file, &b.file)
            .then_with(|| a.line.cmp(&b.line))
            .then_with(|| utf16_cmp(&a.symbol, &b.symbol))
    });
    out.dedup_by(|a, b| {
        a.file == b.file && a.symbol == b.symbol && a.kind == b.kind && a.line == b.line
    });
    out
}

/// Does the specifier name code (no extension, or a script one)?
fn is_script(specifier: &str) -> bool {
    let last = specifier.rsplit('/').next().unwrap_or(specifier);
    match last.rsplit_once('.') {
        None => true,
        Some((_, ext)) => [
            "js", "jsx", "ts", "tsx", "mjs", "cjs", "mts", "cts", "vue", "svelte", "astro",
        ]
        .contains(&ext),
    }
}

/// The file an import of a workspace package reaches: the entry its manifest gives the subpath,
/// else the file the path names inside the package.
fn package_file(
    id: &str,
    specifier: &str,
    apis: &BTreeMap<&str, &PublicApiSurface>,
    boundaries: &BTreeMap<&str, &ModuleBoundary>,
    resolver: &Resolver<'_>,
) -> Option<String> {
    let name = boundaries.get(id)?.name.as_deref()?;
    let rest = specifier.strip_prefix(name)?;
    let key = if rest.is_empty() {
        ".".to_owned()
    } else {
        format!(".{rest}")
    };
    if let Some(api) = apis.get(id)
        && let Some((_, file)) = api.subpaths.iter().find(|(k, _)| *k == key)
    {
        return Some(file.clone());
    }
    if rest.is_empty() {
        return None;
    }
    let from = join(id, "package.json");
    [format!(".{rest}"), format!("./src{rest}")]
        .iter()
        .find_map(|spec| match resolver.resolve(&from, spec) {
            Resolution::File(f) => Some(f),
            _ => None,
        })
}

/// Computed `import(x)` and `require(x)` per module.
fn computed_per_module<'b>(
    ws: &Workspace,
    boundaries: &'b [ModuleBoundary],
    facts: &Facts,
) -> BTreeMap<&'b str, usize> {
    let mut out: BTreeMap<&str, usize> = BTreeMap::new();
    for b in boundaries {
        for file in b.files.iter().filter(|f| facts.contains_key(*f)) {
            let Some(text) = ws.read(file) else { continue };
            let n = super::imports::js_imports(file, text)
                .unwrap_or_default()
                .iter()
                .filter(|r| r.specifier.is_none())
                .count();
            if n > 0 {
                *out.entry(b.id.as_str()).or_default() += n;
            }
        }
    }
    out
}
