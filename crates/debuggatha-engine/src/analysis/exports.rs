//! What a JavaScript or TypeScript file exports and imports by name, and which of its own
//! top-level declarations nothing reads, from the syntax tree and its scopes (oxc). It is the
//! reader the public API and dead code stages share, so both see the same file.
//!
//! A single-file component's script exports nothing a path can reach and its template reads
//! names the script cannot see, so its imports are read and every namespace it imports is
//! treated as used in full; its own declarations are never reported.

use std::collections::{BTreeMap, BTreeSet};

use oxc_allocator::Allocator;
use oxc_ast::AstKind;
use oxc_ast::ast::{
    BindingIdentifier, ComputedMemberExpression, Declaration, ExportAllDeclaration,
    ExportDefaultDeclaration, ExportDefaultDeclarationKind, Expression, IdentifierReference,
    ImportDeclaration, ImportDeclarationSpecifier, ModuleDeclaration, Program, Statement,
    StaticMemberExpression, TSImportEqualsDeclaration, TSModuleReference, TSQualifiedName,
    TSTypeName,
};
use oxc_ast_visit::{Visit, walk};
use oxc_parser::Parser;
use oxc_semantic::{Semantic, SemanticBuilder, SymbolId};

use super::imports::line_starts;
use crate::syntax::oxc::source_type;
use crate::syntax::{Language, script_regions};

/// Where an exported name comes from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Origin {
    /// Declared or bound in this file; the local name when there is one.
    Local(Option<String>),
    /// `export { imported as name } from 'source'` (`imported` is `*` for `export * as name`).
    From { source: String, imported: String },
}

/// How a name is exported.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ExportForm {
    /// `export function f() {}`, `export const a = 1`.
    Declaration,
    /// `export { a, b as c }`.
    Specifier,
    /// `export { a } from 'm'`, or an imported name exported again.
    ReExport,
    /// `export * as ns from 'm'`.
    Namespace,
    Default,
    /// Carried by `export * from 'm'` (only in a public surface).
    Star,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportEntry {
    pub form: ExportForm,
    pub name: String,
    pub line: u32,
    pub origin: Origin,
    pub type_only: bool,
    /// Something in the file other than the export itself reads the declaration.
    pub used_here: bool,
}

/// `export * from 'source'`: every name the source exports, but its default.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StarExport {
    pub source: String,
    pub line: u32,
}

/// How a file uses `import * as ns`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NamespaceUse {
    pub members: BTreeSet<String>,
    /// The namespace is passed around, spread, destructured or exported: every name may be used.
    pub escapes: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ImportFact {
    pub source: String,
    pub line: u32,
    /// Names taken from the source (`default` for the default import).
    pub names: Vec<String>,
    pub namespace: Option<NamespaceUse>,
}

/// A top-level declaration that nothing in its file reads and that is not exported.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unreferenced {
    pub name: String,
    pub line: u32,
    pub kind: &'static str,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FileFacts {
    /// Has an `import` or `export`: a script's top-level names are global, so another file may
    /// read them.
    pub is_module: bool,
    pub exports: Vec<ExportEntry>,
    pub stars: Vec<StarExport>,
    pub imports: Vec<ImportFact>,
    pub unreferenced: Vec<Unreferenced>,
}

fn line_of(starts: &[usize], at: usize) -> u32 {
    u32::try_from(starts.partition_point(|s| *s <= at)).unwrap_or(u32::MAX)
}

/// What the file exports and imports, or `None` when it could not be parsed or is not JS/TS.
#[must_use]
pub fn read_facts(path: &str, source: &str) -> Option<FileFacts> {
    let language = Language::from_path(std::path::Path::new(path))?;
    let starts = line_starts(source);
    match language {
        Language::JavaScript | Language::Jsx | Language::TypeScript | Language::Tsx => {
            let declaration = path.ends_with(".d.ts") || path.ends_with(".d.mts");
            read(source, language, declaration, 0, &starts, true)
        }
        Language::Vue | Language::Svelte | Language::Astro => {
            let mut all = FileFacts::default();
            for region in script_regions(source, language) {
                let code = &source[region.start..region.end];
                if let Some(found) =
                    read(code, region.language, false, region.start, &starts, false)
                {
                    all.imports.extend(found.imports);
                }
            }
            for import in &mut all.imports {
                if let Some(ns) = &mut import.namespace {
                    ns.escapes = true;
                }
            }
            Some(all)
        }
        _ => None,
    }
}

fn read(
    code: &str,
    language: Language,
    declaration_file: bool,
    base: usize,
    starts: &[usize],
    own_names: bool,
) -> Option<FileFacts> {
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, code, source_type(language, declaration_file)).parse();
    if parsed.fatal_error {
        return None;
    }
    let semantic = SemanticBuilder::new()
        .with_build_nodes(true)
        .build(&parsed.program)
        .semantic;
    let at = |offset: u32| line_of(starts, base + offset as usize);

    let mut facts = FileFacts::default();
    let mut exported_locals: BTreeMap<String, usize> = BTreeMap::new();
    let mut namespaces: BTreeMap<SymbolId, usize> = BTreeMap::new();
    let mut bindings: BTreeMap<String, (String, String)> = BTreeMap::new();
    statements(
        &parsed.program,
        &at,
        &mut facts,
        &mut exported_locals,
        &mut namespaces,
        &mut bindings,
    );

    if !namespaces.is_empty() {
        let mut usage = Usage {
            scoping: semantic.scoping(),
            namespaces: &namespaces,
            uses: vec![NamespaceUse::default(); facts.imports.len()],
        };
        usage.visit_program(&parsed.program);
        for (index, ns) in usage.uses.into_iter().enumerate() {
            if let Some(slot) = facts.imports.get_mut(index)
                && slot.namespace.is_some()
            {
                slot.namespace = Some(ns);
            }
        }
    }
    let scoping = semantic.scoping();
    let root = scoping.root_scope_id();
    let mut reads: BTreeMap<&str, usize> = BTreeMap::new();
    for id in scoping
        .symbol_ids()
        .filter(|i| scoping.symbol_scope_id(*i) == root)
    {
        *reads.entry(scoping.symbol_name(id)).or_default() +=
            scoping.get_resolved_reference_ids(id).len();
    }
    for entry in &mut facts.exports {
        let Origin::Local(Some(name)) = &entry.origin else {
            continue;
        };
        let own = exported_locals.get(name).copied().unwrap_or(0);
        let read = reads.get(name.as_str()).copied().unwrap_or(0);
        if let Some((source, imported)) = bindings.get(name)
            && read <= own
        {
            // `import { a } from 'm'; export { a }`: the name is passed on, not read here.
            if imported != "*"
                && let Some(i) = facts.imports.iter_mut().find(|i| i.source == *source)
                && let Some(at) = i.names.iter().position(|n| n == imported)
            {
                i.names.remove(at);
            }
            entry.origin = Origin::From {
                source: source.clone(),
                imported: imported.clone(),
            };
            entry.form = ExportForm::ReExport;
        } else {
            entry.used_here = read > own;
        }
    }
    if own_names && facts.is_module && !declaration_file {
        facts.unreferenced = unreferenced(&semantic, &exported_locals, &at);
    }
    Some(facts)
}

fn push_declared(
    decl: &Declaration<'_>,
    at: &dyn Fn(u32) -> u32,
    facts: &mut FileFacts,
    locals: &mut BTreeMap<String, usize>,
) {
    let mut add = |id: &BindingIdentifier<'_>, type_only: bool| {
        let name = id.name.as_str().to_owned();
        locals.entry(name.clone()).or_default();
        facts.exports.push(ExportEntry {
            form: ExportForm::Declaration,
            line: at(id.span.start),
            origin: Origin::Local(Some(name.clone())),
            name,
            type_only,
            used_here: false,
        });
    };
    match decl {
        Declaration::VariableDeclaration(v) => {
            for d in &v.declarations {
                for id in d.id.get_binding_identifiers() {
                    add(id, false);
                }
            }
        }
        Declaration::FunctionDeclaration(f) => {
            if let Some(id) = &f.id {
                add(id, false);
            }
        }
        Declaration::ClassDeclaration(c) => {
            if let Some(id) = &c.id {
                add(id, false);
            }
        }
        Declaration::TSTypeAliasDeclaration(t) => add(&t.id, true),
        Declaration::TSInterfaceDeclaration(t) => add(&t.id, true),
        Declaration::TSEnumDeclaration(t) => add(&t.id, false),
        Declaration::TSNamespaceDeclaration(t) => add(&t.id, false),
        Declaration::TSImportEqualsDeclaration(t) => add(&t.id, false),
        Declaration::TSExternalModuleDeclaration(_) | Declaration::TSGlobalDeclaration(_) => {}
    }
}

fn import_declaration(
    d: &ImportDeclaration<'_>,
    at: &dyn Fn(u32) -> u32,
    facts: &mut FileFacts,
    namespaces: &mut BTreeMap<SymbolId, usize>,
    bindings: &mut BTreeMap<String, (String, String)>,
) {
    let mut fact = ImportFact {
        source: d.source.value.display().to_string(),
        line: at(d.span.start),
        ..ImportFact::default()
    };
    for spec in d.specifiers.iter().flatten() {
        match spec {
            ImportDeclarationSpecifier::ImportSpecifier(s) => {
                fact.names.push(s.imported.name().to_string());
                bindings.insert(
                    s.local.name.as_str().to_owned(),
                    (fact.source.clone(), s.imported.name().to_string()),
                );
            }
            ImportDeclarationSpecifier::ImportDefaultSpecifier(s) => {
                fact.names.push("default".into());
                bindings.insert(
                    s.local.name.as_str().to_owned(),
                    (fact.source.clone(), "default".into()),
                );
            }
            ImportDeclarationSpecifier::ImportNamespaceSpecifier(s) => {
                bindings.insert(
                    s.local.name.as_str().to_owned(),
                    (fact.source.clone(), "*".into()),
                );
                fact.namespace = Some(NamespaceUse::default());
                namespaces.insert(s.local.symbol_id(), facts.imports.len());
            }
        }
    }
    facts.imports.push(fact);
}

fn export_all(d: &ExportAllDeclaration<'_>, at: &dyn Fn(u32) -> u32, facts: &mut FileFacts) {
    let source = d.source.value.display().to_string();
    if let Some(alias) = &d.exported {
        facts.exports.push(ExportEntry {
            form: ExportForm::Namespace,
            name: alias.name().to_string(),
            line: at(d.span.start),
            origin: Origin::From {
                source: source.clone(),
                imported: "*".into(),
            },
            type_only: d.export_kind.is_type(),
            used_here: false,
        });
        facts.imports.push(escaping(source, at(d.span.start)));
    } else {
        facts.stars.push(StarExport {
            source,
            line: at(d.span.start),
        });
    }
}

fn export_default(
    d: &ExportDefaultDeclaration<'_>,
    at: &dyn Fn(u32) -> u32,
    facts: &mut FileFacts,
    locals: &mut BTreeMap<String, usize>,
) {
    let local = match &d.declaration {
        ExportDefaultDeclarationKind::FunctionDeclaration(f) => {
            f.id.as_ref().map(|i| i.name.as_str().to_owned())
        }
        ExportDefaultDeclarationKind::ClassDeclaration(c) => {
            c.id.as_ref().map(|i| i.name.as_str().to_owned())
        }
        ExportDefaultDeclarationKind::TSInterfaceDeclaration(i) => {
            Some(i.id.name.as_str().to_owned())
        }
        other => match other.as_expression() {
            Some(Expression::Identifier(id)) => Some(id.name.as_str().to_owned()),
            _ => None,
        },
    };
    if let Some(l) = &local {
        *locals.entry(l.clone()).or_default() += 1;
    }
    facts.exports.push(ExportEntry {
        form: ExportForm::Default,
        name: "default".into(),
        line: at(d.span.start),
        origin: Origin::Local(local),
        type_only: false,
        used_here: false,
    });
}

fn statements(
    program: &Program<'_>,
    at: &dyn Fn(u32) -> u32,
    facts: &mut FileFacts,
    locals: &mut BTreeMap<String, usize>,
    namespaces: &mut BTreeMap<SymbolId, usize>,
    bindings: &mut BTreeMap<String, (String, String)>,
) {
    for statement in &program.body {
        if let Statement::TSImportEqualsDeclaration(t) = statement
            && let TSModuleReference::ExternalModuleReference(r) = &t.module_reference
        {
            // `import x = require('m')`: the whole module, however it is used.
            facts.is_module = true;
            facts.imports.push(escaping(
                r.expression.value.display().to_string(),
                at(t.span.start),
            ));
            continue;
        }
        let Some(module) = statement.as_module_declaration() else {
            continue;
        };
        facts.is_module = true;
        match module {
            ModuleDeclaration::ImportDeclaration(d) => {
                import_declaration(d, at, facts, namespaces, bindings);
            }
            ModuleDeclaration::ExportAllDeclaration(d) => export_all(d, at, facts),
            ModuleDeclaration::ExportFromDeclaration(d) => {
                let source = d.source.value.display().to_string();
                // A re-export reads nothing: the name is used when someone imports it from here.
                for s in &d.specifiers {
                    let imported = s.local.name().to_string();
                    facts.exports.push(ExportEntry {
                        form: ExportForm::ReExport,
                        name: s.exported.name().to_string(),
                        line: at(s.span.start),
                        origin: Origin::From {
                            source: source.clone(),
                            imported,
                        },
                        type_only: d.export_kind.is_type() || s.export_kind.is_type(),
                        used_here: false,
                    });
                }
            }
            ModuleDeclaration::ExportNamedDeclaration(d) => {
                for s in &d.specifiers {
                    let local = s.local.name().to_string();
                    *locals.entry(local.clone()).or_default() += 1;
                    facts.exports.push(ExportEntry {
                        form: ExportForm::Specifier,
                        name: s.exported.name().to_string(),
                        line: at(s.span.start),
                        origin: Origin::Local(Some(local)),
                        type_only: d.export_kind.is_type() || s.export_kind.is_type(),
                        used_here: false,
                    });
                }
            }
            ModuleDeclaration::ExportDeclaration(d) => {
                push_declared(&d.declaration, at, facts, locals);
            }
            ModuleDeclaration::ExportDefaultDeclaration(d) => export_default(d, at, facts, locals),
            // `export = x` and `export as namespace X` name no member a path can import.
            ModuleDeclaration::TSExportAssignment(_)
            | ModuleDeclaration::TSNamespaceExportDeclaration(_) => {}
        }
    }
}

/// An import whose use of the module cannot be narrowed to names.
fn escaping(source: String, line: u32) -> ImportFact {
    ImportFact {
        source,
        line,
        names: Vec::new(),
        namespace: Some(NamespaceUse {
            members: BTreeSet::new(),
            escapes: true,
        }),
    }
}

/// How each `import * as ns` of a file is used.
struct Usage<'s> {
    scoping: &'s oxc_semantic::Scoping,
    namespaces: &'s BTreeMap<SymbolId, usize>,
    uses: Vec<NamespaceUse>,
}

impl Usage<'_> {
    fn namespace_of(&self, id: &IdentifierReference<'_>) -> Option<usize> {
        let symbol = self.scoping.get_reference(id.reference_id()).symbol_id()?;
        self.namespaces.get(&symbol).copied()
    }
}

impl<'a> Visit<'a> for Usage<'_> {
    fn visit_identifier_reference(&mut self, it: &IdentifierReference<'a>) {
        // Reached only when the name is not the object of a member access read below.
        if let Some(index) = self.namespace_of(it) {
            self.uses[index].escapes = true;
        }
    }

    fn visit_static_member_expression(&mut self, it: &StaticMemberExpression<'a>) {
        if let Expression::Identifier(id) = &it.object
            && let Some(index) = self.namespace_of(id)
        {
            self.uses[index]
                .members
                .insert(it.property.name.as_str().to_owned());
            return;
        }
        walk::walk_static_member_expression(self, it);
    }

    fn visit_computed_member_expression(&mut self, it: &ComputedMemberExpression<'a>) {
        if let Expression::Identifier(id) = &it.object
            && let Some(index) = self.namespace_of(id)
        {
            if let Expression::StringLiteral(s) = &it.expression {
                self.uses[index]
                    .members
                    .insert(s.value.display().to_string());
            } else {
                self.uses[index].escapes = true;
            }
            self.visit_expression(&it.expression);
            return;
        }
        walk::walk_computed_member_expression(self, it);
    }

    // `ns.Type` in a type position.
    fn visit_ts_qualified_name(&mut self, it: &TSQualifiedName<'a>) {
        if let TSTypeName::IdentifierReference(id) = &it.left
            && let Some(index) = self.namespace_of(id)
        {
            self.uses[index]
                .members
                .insert(it.right.name.as_str().to_owned());
            return;
        }
        walk::walk_ts_qualified_name(self, it);
    }

    fn visit_ts_import_equals_declaration(&mut self, it: &TSImportEqualsDeclaration<'a>) {
        walk::walk_ts_import_equals_declaration(self, it);
    }
}

/// Root-scope declarations of a module that nothing reads. `export`ed names, imports, ambient
/// declarations, names that start with `_` and decorated classes are never listed.
fn unreferenced(
    semantic: &Semantic<'_>,
    exported: &BTreeMap<String, usize>,
    at: &dyn Fn(u32) -> u32,
) -> Vec<Unreferenced> {
    let scoping = semantic.scoping();
    let root = scoping.root_scope_id();
    let mut out = Vec::new();
    for id in scoping.symbol_ids() {
        if scoping.symbol_scope_id(id) != root {
            continue;
        }
        let flags = scoping.symbol_flags(id);
        let name = scoping.symbol_name(id);
        if flags.is_import()
            || flags.is_ambient()
            || name.starts_with('_')
            || exported.contains_key(name)
            || !scoping.get_resolved_reference_ids(id).is_empty()
        {
            continue;
        }
        let kind = if flags.is_function() {
            "function"
        } else if flags.is_class() {
            let decorated = matches!(
                semantic.nodes().kind(scoping.symbol_declaration(id)),
                AstKind::Class(c) if !c.decorators.is_empty()
            );
            if decorated {
                continue;
            }
            "class"
        } else if flags.is_interface() {
            "interface"
        } else if flags.is_type_alias() {
            "type"
        } else if flags.is_enum() {
            "enum"
        } else if flags.is_variable() {
            "variable"
        } else {
            continue;
        };
        out.push(Unreferenced {
            name: name.to_owned(),
            line: at(scoping.symbol_span(id).start),
            kind,
        });
    }
    out.sort_by(|a, b| a.line.cmp(&b.line).then_with(|| a.name.cmp(&b.name)));
    out
}

/// The facts of every JavaScript, TypeScript and single-file-component file of a workspace.
pub type Facts = BTreeMap<String, FileFacts>;

/// Reads every such file the workspace owns. A file that does not parse has no entry: nothing is
/// concluded about it.
#[must_use]
pub fn read_all(ws: &super::workspace::Workspace) -> Facts {
    let mut out = Facts::new();
    for path in ws.files_with(&[
        "ts", "tsx", "mts", "cts", "js", "jsx", "mjs", "cjs", "vue", "svelte", "astro",
    ]) {
        if let Some(text) = ws.read(path)
            && let Some(facts) = read_facts(path, text)
        {
            out.insert(path.to_owned(), facts);
        }
    }
    out
}

/// Follows `export { x } from` and `export *` from file to file, to where a name is declared.
pub struct ExportGraph<'a> {
    pub facts: &'a Facts,
    pub resolver: &'a super::resolve::Resolver<'a>,
}

impl ExportGraph<'_> {
    fn target(&self, from: &str, source: &str) -> Option<String> {
        match self.resolver.resolve(from, source) {
            super::resolve::Resolution::File(f) => Some(f),
            _ => None,
        }
    }

    /// The `(file, name)` where `name`, exported by `file`, is declared (or bound to a namespace).
    #[must_use]
    pub fn definition(&self, file: &str, name: &str) -> Option<(String, String)> {
        self.define(file, name, &mut BTreeSet::new())
    }

    fn define(
        &self,
        file: &str,
        name: &str,
        seen: &mut BTreeSet<(String, String)>,
    ) -> Option<(String, String)> {
        if !seen.insert((file.to_owned(), name.to_owned())) {
            return None;
        }
        let facts = self.facts.get(file)?;
        if let Some(entry) = facts.exports.iter().find(|e| e.name == name) {
            return match &entry.origin {
                Origin::Local(_) => Some((file.to_owned(), name.to_owned())),
                Origin::From { source, imported } => {
                    if imported == "*" {
                        return Some((file.to_owned(), name.to_owned()));
                    }
                    let next = self.target(file, source)?;
                    self.define(&next, imported, seen)
                }
            };
        }
        if name == "default" {
            return None;
        }
        for star in &facts.stars {
            if let Some(next) = self.target(file, &star.source)
                && let Some(found) = self.define(&next, name, seen)
            {
                return Some(found);
            }
        }
        None
    }

    /// Every name the file exports, `export *` followed (it never carries `default`).
    #[must_use]
    pub fn names(&self, file: &str) -> BTreeSet<String> {
        let mut out = BTreeSet::new();
        self.collect(file, &mut BTreeSet::new(), &mut out, true);
        out
    }

    fn collect(
        &self,
        file: &str,
        seen: &mut BTreeSet<String>,
        out: &mut BTreeSet<String>,
        with_default: bool,
    ) {
        if !seen.insert(file.to_owned()) {
            return;
        }
        let Some(facts) = self.facts.get(file) else {
            return;
        };
        for e in &facts.exports {
            if with_default || e.name != "default" {
                out.insert(e.name.clone());
            }
        }
        for star in &facts.stars {
            if let Some(next) = self.target(file, &star.source) {
                self.collect(&next, seen, out, false);
            }
        }
    }
}
