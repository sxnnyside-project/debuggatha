//! The public surface of a Rust library crate: what `pub` reaches from `lib.rs`, read from the
//! syntax tree (tree-sitter). An item is public when it is `pub` (not `pub(crate)`, `pub(super)`
//! or `pub(in ..)`) and every module on its path is `pub`; `pub use` re-exports a name from where
//! it is declared, and `#[macro_export]` puts a macro at the crate root. A `pub` item in a module
//! no `pub` path reaches, and that no `pub use` re-exports, is "unexpectedly exposed": the
//! compiler lets nothing outside the crate name it (`unreachable_pub`).
//!
//! Not read: the associated items of a type (methods, fields, variants), `impl` blocks, macro
//! bodies that declare items, `cfg` (every branch counts, `#[cfg(test)]` modules do not), and glob
//! re-exports of another crate. A binary crate has no surface.

use std::collections::BTreeSet;

use tree_sitter::{Node, Parser};

use super::exports::ExportForm;
use super::public_api::{PublicApiSymbol, Stability};
use crate::analysis::structure::rust::{line, path_attribute, path_segments, text};
use crate::analysis::structure::workspace::{Workspace, dirname, join};
use crate::syntax::Language;
use crate::syntax::treesitter::grammar;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DefKind {
    Item,
    Module,
}

#[derive(Debug, Clone)]
struct Def {
    module: Vec<String>,
    name: String,
    file: String,
    line: u32,
    public: bool,
    reachable: bool,
    hidden: bool,
    kind: DefKind,
}

#[derive(Debug, Clone)]
struct Use {
    module: Vec<String>,
    path: Vec<String>,
    alias: Option<String>,
    glob: bool,
    public: bool,
    reachable: bool,
    file: String,
    line: u32,
}

/// Where the items being read sit.
struct Here<'a> {
    file: &'a str,
    module: &'a [String],
    reachable: bool,
    dir: &'a str,
}

#[derive(Default)]
struct Crate {
    defs: Vec<Def>,
    uses: Vec<Use>,
    modules: BTreeSet<Vec<String>>,
    seen: BTreeSet<String>,
}

fn is_public(item: Node<'_>, src: &str) -> bool {
    let mut c = item.walk();
    item.children(&mut c)
        .any(|k| k.kind() == "visibility_modifier" && text(k, src).trim() == "pub")
}

/// The text of the attributes right before `item`.
fn attributes(item: Node<'_>, src: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut prev = item.prev_named_sibling();
    while let Some(p) = prev {
        if p.kind() != "attribute_item" {
            break;
        }
        out.push(text(p, src).replace(' ', ""));
        prev = p.prev_named_sibling();
    }
    out
}

fn use_items(
    node: Node<'_>,
    prefix: &[String],
    alias: Option<&str>,
    src: &str,
    out: &mut Vec<(Vec<String>, Option<String>, bool)>,
) {
    match node.kind() {
        "identifier" | "crate" | "self" | "super" | "scoped_identifier" => {
            if let Some(segs) = path_segments(node, src) {
                let mut full = prefix.to_vec();
                if segs != ["self"] || prefix.is_empty() {
                    full.extend(segs);
                }
                out.push((full, alias.map(str::to_owned), false));
            }
        }
        "use_as_clause" => {
            let name = node.child_by_field_name("alias").map(|a| text(a, src));
            if let Some(p) = node.child_by_field_name("path") {
                use_items(p, prefix, name, src, out);
            }
        }
        "use_wildcard" => {
            let mut c = node.walk();
            let mut full = prefix.to_vec();
            if let Some(p) = node.named_children(&mut c).next()
                && let Some(segs) = path_segments(p, src)
            {
                full.extend(segs);
            }
            out.push((full, None, true));
        }
        "scoped_use_list" => {
            let mut full = prefix.to_vec();
            if let Some(p) = node.child_by_field_name("path")
                && let Some(segs) = path_segments(p, src)
            {
                full.extend(segs);
            }
            if let Some(list) = node.child_by_field_name("list") {
                use_items(list, &full, None, src, out);
            }
        }
        "use_list" => {
            let mut c = node.walk();
            for child in node.named_children(&mut c) {
                use_items(child, prefix, None, src, out);
            }
        }
        _ => {}
    }
}

fn stem(file: &str) -> &str {
    let name = file.rsplit('/').next().unwrap_or(file);
    name.strip_suffix(".rs").unwrap_or(name)
}

impl Crate {
    /// Reads the items of `body` (a file's root or an inline module's block).
    fn items(&mut self, ws: &Workspace, body: Node<'_>, src: &str, here: &Here<'_>) {
        let mut cursor = body.walk();
        for item in body.named_children(&mut cursor) {
            let attrs = attributes(item, src);
            if attrs.iter().any(|a| a.contains("cfg(test)")) {
                continue;
            }
            let public = is_public(item, src);
            let hidden = attrs.iter().any(|a| a.contains("doc(hidden)"));
            let name = item
                .child_by_field_name("name")
                .map(|n| text(n, src).to_owned());
            let def =
                |name: String, module: Vec<String>, public: bool, reachable: bool, kind| Def {
                    module,
                    name,
                    file: here.file.to_owned(),
                    line: line(item),
                    public,
                    reachable,
                    hidden,
                    kind,
                };
            match item.kind() {
                "function_item" | "struct_item" | "enum_item" | "union_item" | "trait_item"
                | "type_item" | "const_item" | "static_item" => {
                    if let Some(name) = name {
                        let d = def(
                            name,
                            here.module.to_vec(),
                            public,
                            here.reachable,
                            DefKind::Item,
                        );
                        self.defs.push(d);
                    }
                }
                "macro_definition" => {
                    if let Some(name) = name
                        && attrs.iter().any(|a| a.contains("macro_export"))
                    {
                        // `#[macro_export]` places the macro at the crate root.
                        self.defs
                            .push(def(name, Vec::new(), true, true, DefKind::Item));
                    }
                }
                "use_declaration" => {
                    let mut found = Vec::new();
                    if let Some(arg) = item.child_by_field_name("argument") {
                        use_items(arg, &[], None, src, &mut found);
                    }
                    for (path, alias, glob) in found {
                        self.uses.push(Use {
                            module: here.module.to_vec(),
                            path,
                            alias,
                            glob,
                            public,
                            reachable: here.reachable,
                            file: here.file.to_owned(),
                            line: line(item),
                        });
                    }
                }
                "mod_item" => {
                    if let Some(name) = name {
                        if public {
                            let d = def(
                                name.clone(),
                                here.module.to_vec(),
                                public,
                                here.reachable,
                                DefKind::Module,
                            );
                            self.defs.push(d);
                        }
                        self.module_item(ws, item, src, here, &name, public);
                    }
                }
                _ => {}
            }
        }
    }

    /// Follows a `mod` into its block or its file.
    fn module_item(
        &mut self,
        ws: &Workspace,
        item: Node<'_>,
        src: &str,
        here: &Here<'_>,
        name: &str,
        public: bool,
    ) {
        let mut inner = here.module.to_vec();
        inner.push(name.to_owned());
        self.modules.insert(inner.clone());
        let reachable = here.reachable && public;
        let dir = join(here.dir, name);
        if let Some(block) = item.child_by_field_name("body") {
            let next = Here {
                file: here.file,
                module: &inner,
                reachable,
                dir: &dir,
            };
            self.items(ws, block, src, &next);
            return;
        }
        let candidates = match path_attribute(item, src) {
            Some(p) => vec![join(dirname(here.file), &p)],
            None => vec![
                join(here.dir, &format!("{name}.rs")),
                join(here.dir, &format!("{name}/mod.rs")),
            ],
        };
        if let Some((child, text)) = candidates
            .into_iter()
            .find_map(|c| ws.read(&c).map(|t| (c, t)))
        {
            self.file(ws, &child, text, &inner, reachable);
        }
    }

    fn file(
        &mut self,
        ws: &Workspace,
        file: &str,
        source: &str,
        module: &[String],
        reachable: bool,
    ) {
        if !self.seen.insert(file.to_owned()) {
            return;
        }
        let mut parser = Parser::new();
        let Some(lang) = grammar(Language::Rust) else {
            return;
        };
        if parser.set_language(&lang).is_err() {
            return;
        }
        let Some(tree) = parser.parse(source, None) else {
            return;
        };
        let top = module.is_empty() || stem(file) == "mod";
        let dir = if top {
            dirname(file).to_owned()
        } else {
            join(dirname(file), stem(file))
        };
        let here = Here {
            file,
            module,
            reachable,
            dir: &dir,
        };
        self.items(ws, tree.root_node(), source, &here);
    }

    /// Where a path written in `from` leads: the module it reaches and the segments left over (the
    /// item). With `to_module`, every segment is a module (`use a::b::*`).
    fn locate<'p>(
        &self,
        from: &[String],
        path: &'p [String],
        to_module: bool,
    ) -> Option<(Vec<String>, &'p [String])> {
        let mut at: Vec<String> = from.to_vec();
        let mut rest = path;
        match rest.first().map(String::as_str)? {
            "crate" => {
                at.clear();
                rest = &rest[1..];
            }
            "self" => rest = &rest[1..],
            "super" => {
                while rest.first().map(String::as_str) == Some("super") {
                    at.pop()?;
                    rest = &rest[1..];
                }
            }
            first => {
                let mut child = at.clone();
                child.push(first.to_owned());
                // Neither a module here nor an item here: another crate.
                let item_here = rest.len() == 1 && self.has(&at, first);
                if !(self.modules.contains(&child) || item_here) {
                    return None;
                }
            }
        }
        while let Some(seg) = rest.first() {
            if rest.len() == 1 && !to_module {
                break;
            }
            let mut child = at.clone();
            child.push(seg.clone());
            if !self.modules.contains(&child) {
                break;
            }
            at = child;
            rest = &rest[1..];
        }
        Some((at, rest))
    }

    fn has(&self, module: &[String], name: &str) -> bool {
        self.defs
            .iter()
            .any(|d| d.module == module && d.name == name)
            || self
                .uses
                .iter()
                .any(|u| u.module == module && !u.glob && exported_name(u) == name)
    }

    /// The declaration `name` of `module` stands for, through `use`.
    fn definition(&self, module: &[String], name: &str, depth: u8) -> Option<&Def> {
        if let Some(d) = self
            .defs
            .iter()
            .find(|d| d.module == module && d.name == name)
        {
            return Some(d);
        }
        if depth == 0 {
            return None;
        }
        for u in self.uses.iter().filter(|u| u.module == module && !u.glob) {
            if exported_name(u) == name
                && let Some((m, rest)) = self.locate(module, &u.path, false)
                && let [item] = rest
                && let Some(d) = self.definition(&m, item, depth - 1)
            {
                return Some(d);
            }
        }
        None
    }
}

fn exported_name(u: &Use) -> String {
    u.alias
        .clone()
        .or_else(|| u.path.last().cloned())
        .unwrap_or_default()
}

fn qualified(d: &Def) -> String {
    let mut parts = d.module.clone();
    parts.push(d.name.clone());
    parts.join("::")
}

fn symbol(d: &Def, name: &str, file: &str, line: u32, form: ExportForm) -> PublicApiSymbol {
    PublicApiSymbol {
        name: name.to_owned(),
        file: file.to_owned(),
        line,
        stability: if d.hidden || d.name.starts_with('_') {
            Stability::Unstable
        } else {
            Stability::Stable
        },
        form,
        definition: Some((d.file.clone(), qualified(d))),
    }
}

/// A module made public by a `pub use` (or a glob) brings its `pub` contents with it.
fn add_module_contents(
    krate: &Crate,
    exported: &mut Vec<PublicApiSymbol>,
    public_defs: &mut BTreeSet<(String, String)>,
) {
    let roots: Vec<(String, Vec<String>)> = exported
        .iter()
        .filter_map(|s| {
            let (file, q) = s.definition.as_ref()?;
            let d = krate
                .defs
                .iter()
                .find(|d| d.kind == DefKind::Module && d.file == *file && qualified(d) == *q)?;
            let mut at = d.module.clone();
            at.push(d.name.clone());
            Some((s.name.clone(), at))
        })
        .collect();
    for (shown, prefix) in roots {
        for d in krate
            .defs
            .iter()
            .filter(|d| d.public && d.module.len() >= prefix.len() && d.module.starts_with(&prefix))
        {
            // Every module between the prefix and the item must be `pub`.
            let through_pub = (prefix.len()..d.module.len()).all(|i| {
                krate.defs.iter().any(|m| {
                    m.kind == DefKind::Module
                        && m.public
                        && m.module == d.module[..i]
                        && m.name == d.module[i]
                })
            });
            if !through_pub {
                continue;
            }
            let mut shown_name = vec![shown.clone()];
            shown_name.extend(d.module[prefix.len()..].iter().cloned());
            shown_name.push(d.name.clone());
            let shown_name = shown_name.join("::");
            if exported.iter().any(|e| e.name == shown_name) {
                continue;
            }
            public_defs.insert((d.file.clone(), qualified(d)));
            exported.push(symbol(
                d,
                &shown_name,
                &d.file,
                d.line,
                ExportForm::ReExport,
            ));
        }
    }
}

/// The surface of the library crate rooted at `lib_root`: its public names, and the `pub` items
/// nothing public reaches.
#[must_use]
pub fn rust_surface(
    ws: &Workspace,
    lib_root: &str,
) -> Option<(Vec<PublicApiSymbol>, Vec<PublicApiSymbol>)> {
    let source = ws.read(lib_root)?;
    let mut krate = Crate::default();
    krate.file(ws, lib_root, source, &[], true);

    let mut exported: Vec<PublicApiSymbol> = Vec::new();
    let mut public_defs: BTreeSet<(String, String)> = BTreeSet::new();
    for d in krate.defs.iter().filter(|d| d.public && d.reachable) {
        let name = qualified(d);
        public_defs.insert((d.file.clone(), name.clone()));
        exported.push(symbol(d, &name, &d.file, d.line, ExportForm::Declaration));
    }
    for u in krate.uses.iter().filter(|u| u.public && u.reachable) {
        let located = krate.locate(&u.module, &u.path, u.glob);
        if u.glob {
            // `pub use a::*`: every `pub` item declared in module `a`.
            if let Some((m, rest)) = located
                && rest.is_empty()
            {
                for d in krate.defs.iter().filter(|d| d.module == m && d.public) {
                    let mut at = u.module.clone();
                    at.push(d.name.clone());
                    public_defs.insert((d.file.clone(), qualified(d)));
                    exported.push(symbol(d, &at.join("::"), &u.file, u.line, ExportForm::Star));
                }
            }
            continue;
        }
        let name = exported_name(u);
        if name == "_" || name.is_empty() {
            continue;
        }
        let mut shown = u.module.clone();
        shown.push(name);
        let shown = shown.join("::");
        let def = located.and_then(|(m, rest)| match rest {
            [item] => krate.definition(&m, item, 8),
            _ => None,
        });
        match def {
            Some(d) => {
                public_defs.insert((d.file.clone(), qualified(d)));
                exported.push(symbol(d, &shown, &u.file, u.line, ExportForm::ReExport));
            }
            // An item of another crate, re-exported: a public name whose declaration is elsewhere.
            None => exported.push(PublicApiSymbol {
                name: shown,
                file: u.file.clone(),
                line: u.line,
                stability: Stability::Stable,
                form: ExportForm::ReExport,
                definition: None,
            }),
        }
    }
    add_module_contents(&krate, &mut exported, &mut public_defs);
    let mut exposed = Vec::new();
    for d in krate.defs.iter().filter(|d| d.public && !d.reachable) {
        if !public_defs.contains(&(d.file.clone(), qualified(d))) {
            exposed.push(symbol(
                d,
                &qualified(d),
                &d.file,
                d.line,
                ExportForm::Declaration,
            ));
        }
    }
    let order = |a: &PublicApiSymbol, b: &PublicApiSymbol| {
        debuggatha_core::js::utf16_cmp(&a.file, &b.file)
            .then_with(|| a.line.cmp(&b.line))
            .then_with(|| a.name.cmp(&b.name))
    };
    exported.sort_by(order);
    exposed.sort_by(order);
    Some((exported, exposed))
}
