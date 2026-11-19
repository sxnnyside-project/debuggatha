//! Rust: the module tree of each crate and the paths a file uses, read from tree-sitter.
//!
//! A crate's modules are found the way the compiler finds them: from each root (`src/lib.rs`,
//! `src/main.rs`, `src/bin/*`, `build.rs`, `tests/*`, `examples/*`, `benches/*`, or the paths a
//! manifest names) through `mod x;` to `x.rs` or `x/mod.rs` (or a `#[path]`), and through inline
//! `mod x { .. }`. A path (`use` or an expression) then resolves to the file of the deepest module
//! it names, or to a crate of the workspace by its name. Not read: macro bodies (they are tokens),
//! `cfg` (every `mod` counts), globs of items (`use a::*` names module `a`), `use` of a name
//! brought in by another `use` (re-exports are followed to the module, not the item).

use std::collections::BTreeMap;

use tree_sitter::{Node, Parser};

use super::workspace::{Workspace, dirname, join};
use crate::syntax::Language;
use crate::syntax::treesitter::grammar;

/// A path a file writes, with where it was written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RustRef {
    /// The path, split at `::`.
    pub segments: Vec<String>,
    /// The line.
    pub line: u32,
    /// The inline modules the reference sits in, outermost first.
    pub inline: Vec<String>,
    /// A `mod x;` line: it names a child file, it does not use a path.
    pub decl: bool,
}

/// A `mod` declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModDecl {
    /// The module's name.
    pub name: String,
    /// The `#[path]` attribute, when the declaration has one.
    pub path_attr: Option<String>,
    /// `Some` for `mod x { .. }`.
    pub body: Option<Vec<ModDecl>>,
}

/// What one Rust file declares and refers to.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RustFile {
    /// The `mod` declarations.
    pub mods: Vec<ModDecl>,
    /// The paths it writes.
    pub refs: Vec<RustRef>,
}

pub(in crate::analysis) fn text<'s>(n: Node<'_>, src: &'s str) -> &'s str {
    &src[n.start_byte()..n.end_byte()]
}

pub(in crate::analysis) fn line(n: Node<'_>) -> u32 {
    u32::try_from(n.start_position().row + 1).unwrap_or(u32::MAX)
}

/// The value of `#[path = "x"]` among the attributes right before `node`.
pub(in crate::analysis) fn path_attribute(node: Node<'_>, src: &str) -> Option<String> {
    let mut prev = node.prev_named_sibling();
    while let Some(p) = prev {
        if p.kind() != "attribute_item" {
            break;
        }
        let mut c = p.walk();
        for attr in p.named_children(&mut c).filter(|a| a.kind() == "attribute") {
            let mut ac = attr.walk();
            let kids: Vec<_> = attr.named_children(&mut ac).collect();
            if kids.first().map(|k| text(*k, src)) == Some("path")
                && let Some(value) = attr.child_by_field_name("value")
            {
                let raw = text(value, src);
                return Some(raw.trim_matches('"').to_owned());
            }
        }
        prev = p.prev_named_sibling();
    }
    None
}

fn mod_decls(parent: Node<'_>, src: &str) -> Vec<ModDecl> {
    let mut out = Vec::new();
    let mut cursor = parent.walk();
    for item in parent.named_children(&mut cursor) {
        if item.kind() != "mod_item" {
            continue;
        }
        let Some(name) = item.child_by_field_name("name") else {
            continue;
        };
        out.push(ModDecl {
            name: text(name, src).to_owned(),
            path_attr: path_attribute(item, src),
            body: item.child_by_field_name("body").map(|b| mod_decls(b, src)),
        });
    }
    out
}

/// The segments of a path node (`a::b::c`), or `None` when it is not a plain path.
pub(in crate::analysis) fn path_segments(node: Node<'_>, src: &str) -> Option<Vec<String>> {
    match node.kind() {
        "identifier" | "crate" | "self" | "super" | "type_identifier" => {
            Some(vec![text(node, src).to_owned()])
        }
        "scoped_identifier" | "scoped_type_identifier" => {
            let mut segs = match node.child_by_field_name("path") {
                Some(p) => path_segments(p, src)?,
                None => Vec::new(),
            };
            segs.push(text(node.child_by_field_name("name")?, src).to_owned());
            Some(segs)
        }
        // `Vec<T>::new`, `<T as Trait>::f`: not a module path.
        _ => None,
    }
}

/// Every path a `use` argument names, with `prefix` already in front.
fn use_paths(node: Node<'_>, prefix: &[String], src: &str, out: &mut Vec<Vec<String>>) {
    match node.kind() {
        "identifier" | "crate" | "self" | "super" | "scoped_identifier" => {
            if let Some(segs) = path_segments(node, src) {
                let mut full = prefix.to_vec();
                // `a::{self}` names `a` itself.
                if segs != ["self"] || prefix.is_empty() {
                    full.extend(segs);
                }
                out.push(full);
            }
        }
        "use_as_clause" => {
            if let Some(p) = node.child_by_field_name("path") {
                use_paths(p, prefix, src, out);
            }
        }
        "use_wildcard" => {
            let mut c = node.walk();
            if let Some(p) = node.named_children(&mut c).next() {
                use_paths(p, prefix, src, out);
            } else if !prefix.is_empty() {
                out.push(prefix.to_vec());
            }
        }
        "scoped_use_list" => {
            let mut full = prefix.to_vec();
            if let Some(p) = node.child_by_field_name("path")
                && let Some(segs) = path_segments(p, src)
            {
                full.extend(segs);
            }
            if let Some(list) = node.child_by_field_name("list") {
                use_paths(list, &full, src, out);
            }
        }
        "use_list" => {
            let mut c = node.walk();
            for child in node.named_children(&mut c) {
                use_paths(child, prefix, src, out);
            }
        }
        _ => {}
    }
}

fn collect_refs(node: Node<'_>, src: &str, inline: &mut Vec<String>, out: &mut Vec<RustRef>) {
    match node.kind() {
        "use_declaration" => {
            if let Some(arg) = node.child_by_field_name("argument") {
                let mut paths = Vec::new();
                use_paths(arg, &[], src, &mut paths);
                for segments in paths {
                    out.push(RustRef {
                        segments,
                        line: line(node),
                        inline: inline.clone(),
                        decl: false,
                    });
                }
            }
            return;
        }
        "extern_crate_declaration" => {
            if let Some(name) = node.child_by_field_name("name") {
                out.push(RustRef {
                    segments: vec![text(name, src).to_owned()],
                    line: line(node),
                    inline: inline.clone(),
                    decl: false,
                });
            }
            return;
        }
        "scoped_identifier" | "scoped_type_identifier" => {
            if let Some(segments) = path_segments(node, src) {
                out.push(RustRef {
                    segments,
                    line: line(node),
                    inline: inline.clone(),
                    decl: false,
                });
                return;
            }
        }
        "mod_item" => {
            let name_node = node.child_by_field_name("name");
            if node.child_by_field_name("body").is_none()
                && let Some(name) = name_node
            {
                out.push(RustRef {
                    segments: vec![text(name, src).to_owned()],
                    line: line(node),
                    inline: inline.clone(),
                    decl: true,
                });
            }
            if let Some(body) = node.child_by_field_name("body") {
                let name = node
                    .child_by_field_name("name")
                    .map(|n| text(n, src).to_owned())
                    .unwrap_or_default();
                inline.push(name);
                let mut c = body.walk();
                for child in body.named_children(&mut c) {
                    collect_refs(child, src, inline, out);
                }
                inline.pop();
            }
            return;
        }
        _ => {}
    }
    let mut c = node.walk();
    for child in node.named_children(&mut c) {
        collect_refs(child, src, inline, out);
    }
}

/// Reads one Rust file; `None` when it could not be parsed at all.
#[must_use]
pub fn read_rust(source: &str) -> Option<RustFile> {
    let mut parser = Parser::new();
    parser.set_language(&grammar(Language::Rust)?).ok()?;
    let tree = parser.parse(source, None)?;
    let root = tree.root_node();
    let mut refs = Vec::new();
    let mut c = root.walk();
    for child in root.named_children(&mut c) {
        collect_refs(child, source, &mut Vec::new(), &mut refs);
    }
    Some(RustFile {
        mods: mod_decls(root, source),
        refs,
    })
}

/// One module of a crate.
#[derive(Debug, Clone)]
struct Module {
    file: String,
    parent: Option<usize>,
    children: BTreeMap<String, usize>,
}

/// The modules of every crate of the workspace, and which module each file is.
#[derive(Debug, Default)]
pub struct ModuleTrees {
    modules: Vec<Module>,
    /// file → the module the file is the top of.
    by_file: BTreeMap<String, usize>,
    /// module → the crate root module it belongs to.
    roots: Vec<usize>,
}

fn stem(file: &str) -> &str {
    let name = file.rsplit('/').next().unwrap_or(file);
    name.strip_suffix(".rs").unwrap_or(name)
}

impl ModuleTrees {
    /// Builds the trees of every crate found in the workspace.
    #[must_use]
    pub fn build(ws: &Workspace, files: &BTreeMap<String, RustFile>) -> Self {
        let mut trees = Self::default();
        for root in crate_roots(ws) {
            if files.contains_key(&root) && !trees.by_file.contains_key(&root) {
                trees.add_file(&root, None, None, files);
            }
        }
        trees
    }

    /// Adds the module whose file is `file` and everything it declares. `dir` is where its child
    /// files live.
    fn add_file(
        &mut self,
        file: &str,
        parent: Option<usize>,
        root: Option<usize>,
        files: &BTreeMap<String, RustFile>,
    ) {
        let id = self.modules.len();
        self.modules.push(Module {
            file: file.to_owned(),
            parent,
            children: BTreeMap::new(),
        });
        let root = root.unwrap_or(id);
        self.roots.push(root);
        self.by_file.insert(file.to_owned(), id);
        let Some(parsed) = files.get(file) else {
            return;
        };
        let base = if parent.is_none() || stem(file) == "mod" {
            dirname(file).to_owned()
        } else {
            format!("{}/{}", dirname(file), stem(file))
                .trim_start_matches('/')
                .to_owned()
        };
        self.add_decls(&parsed.mods, id, file, &base, dirname(file), root, files);
    }

    #[allow(clippy::too_many_arguments)] // one recursive walk; a struct would only rename the arguments
    fn add_decls(
        &mut self,
        decls: &[ModDecl],
        parent: usize,
        file: &str,
        dir: &str,
        path_base: &str,
        root: usize,
        files: &BTreeMap<String, RustFile>,
    ) {
        for decl in decls {
            if let Some(body) = &decl.body {
                let id = self.modules.len();
                self.modules.push(Module {
                    file: file.to_owned(),
                    parent: Some(parent),
                    children: BTreeMap::new(),
                });
                self.roots.push(root);
                self.modules[parent].children.insert(decl.name.clone(), id);
                let inner = format!("{dir}/{}", decl.name)
                    .trim_start_matches('/')
                    .to_owned();
                self.add_decls(body, id, file, &inner, &inner, root, files);
                continue;
            }
            let candidates = match &decl.path_attr {
                Some(p) => vec![join(path_base, p)],
                None => vec![
                    join(dir, &format!("{}.rs", decl.name)),
                    join(dir, &format!("{}/mod.rs", decl.name)),
                ],
            };
            if let Some(child) = candidates.into_iter().find(|c| files.contains_key(c)) {
                if self.by_file.contains_key(&child) {
                    // Two `mod` lines naming one file (`#[cfg]` variants): the module exists once.
                    if let Some(existing) = self.by_file.get(&child) {
                        self.modules[parent]
                            .children
                            .insert(decl.name.clone(), *existing);
                    }
                    continue;
                }
                let next = self.modules.len();
                self.modules[parent]
                    .children
                    .insert(decl.name.clone(), next);
                self.add_file(&child, Some(parent), Some(root), files);
            }
        }
    }

    /// Is the file the top of a module of some crate?
    #[must_use]
    pub fn contains_file(&self, file: &str) -> bool {
        self.by_file.contains_key(file)
    }

    /// The module a reference sits in: the file's top module and the inline modules inside it.
    fn locate(&self, file: &str, inline: &[String]) -> Option<usize> {
        let mut at = *self.by_file.get(file)?;
        for name in inline {
            at = *self.modules[at].children.get(name)?;
        }
        Some(at)
    }

    /// The file of the module the path names, or `None` when its first segment is not a module
    /// path (a crate, a type, a function).
    #[must_use]
    pub fn resolve(&self, file: &str, r: &RustRef) -> Option<String> {
        let here = self.locate(file, &r.inline)?;
        let mut segs = r.segments.as_slice();
        let mut at = match segs.first().map(String::as_str)? {
            "crate" => {
                segs = &segs[1..];
                self.roots[here]
            }
            "self" => {
                segs = &segs[1..];
                here
            }
            "super" => {
                let mut at = here;
                while segs.first().map(String::as_str) == Some("super") {
                    at = self.modules[at].parent?;
                    segs = &segs[1..];
                }
                at
            }
            first => {
                let child = *self.modules[here].children.get(first)?;
                segs = &segs[1..];
                child
            }
        };
        for seg in segs {
            match self.modules[at].children.get(seg) {
                Some(next) => at = *next,
                None => break,
            }
        }
        Some(self.modules[at].file.clone())
    }

    /// Does the path start with a module of this crate (rather than naming another crate)?
    #[must_use]
    pub fn is_local(&self, file: &str, r: &RustRef) -> bool {
        let Some(here) = self.locate(file, &r.inline) else {
            return false;
        };
        match r.segments.first().map(String::as_str) {
            Some("crate" | "self" | "super") => true,
            Some(first) => self.modules[here].children.contains_key(first),
            None => false,
        }
    }
}

/// The files a crate starts from, by Cargo's conventions and the paths its manifest names.
fn crate_roots(ws: &Workspace) -> Vec<String> {
    let mut roots = Vec::new();
    let manifests: Vec<String> = ws
        .files_with(&["toml"])
        .into_iter()
        .filter(|p| p.rsplit('/').next() == Some("Cargo.toml"))
        .map(str::to_owned)
        .collect();
    for manifest in manifests {
        let dir = dirname(&manifest).to_owned();
        let at = |rel: &str| join(&dir, rel);
        let mut push = |p: String| {
            if ws.exists(&p) && !roots.contains(&p) {
                roots.push(p);
            }
        };
        push(at("src/lib.rs"));
        push(at("src/main.rs"));
        push(at("build.rs"));
        for sub in ["src/bin", "tests", "examples", "benches"] {
            let base = at(sub);
            for f in ws.files_with(&["rs"]) {
                let Some(rest) = f.strip_prefix(&format!("{base}/")) else {
                    continue;
                };
                // `bin/x.rs` and `bin/x/main.rs` are targets; deeper files are their modules.
                if !rest.contains('/')
                    || (rest.matches('/').count() == 1 && rest.ends_with("/main.rs"))
                {
                    push(f.to_owned());
                }
            }
        }
        if let Some(text) = ws.read(&manifest)
            && let Ok(doc) = text.parse::<toml::Table>()
        {
            if let Some(p) = doc
                .get("lib")
                .and_then(|l| l.get("path"))
                .and_then(|p| p.as_str())
            {
                push(at(p));
            }
            if let Some(bins) = doc.get("bin").and_then(|b| b.as_array()) {
                for b in bins {
                    if let Some(p) = b.get("path").and_then(|p| p.as_str()) {
                        push(at(p));
                    }
                }
            }
        }
    }
    roots
}
