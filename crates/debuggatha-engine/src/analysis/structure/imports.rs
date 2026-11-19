//! The module references a source file makes: `import`, `export ... from`, `import()`,
//! `require()`, `import x = require()` and `import('x').T` types, read from the syntax tree (oxc),
//! so a line in a comment or a string is not an import and a statement over several lines is one.
//! A reference whose specifier is not a literal (`import(name)`) is kept with `specifier: None`:
//! the graph records it as unresolvable instead of dropping it.

use oxc_allocator::Allocator;
use oxc_ast::ast::{
    CallExpression, ExportAllDeclaration, ExportFromDeclaration, Expression, ImportDeclaration,
    ImportExpression, ImportOrExportKind, TSImportEqualsDeclaration, TSImportType,
    TSModuleReference,
};
use oxc_ast_visit::{Visit, walk};
use oxc_parser::Parser;
use oxc_span::GetSpan;

use crate::syntax::oxc::source_type;
use crate::syntax::{Language, script_regions};

/// How a file refers to another module.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportKind {
    /// `import x from`, `import 'x'`, `import x = require()` in a module.
    Static,
    /// `export ... from 'x'`.
    ReExport,
    /// `import('x')`.
    Dynamic,
    /// `require('x')`.
    Require,
    /// `import('x').T` in a type position.
    TypeQuery,
}

/// One reference to a module made by a source file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportRef {
    /// `None` when the specifier is computed.
    pub specifier: Option<String>,
    /// 1-based line of the statement or call.
    pub line: u32,
    /// How the file refers to it.
    pub kind: ImportKind,
    /// `import type`, `export type`: erased at build, still a dependency of the source.
    pub type_only: bool,
}

struct Collector<'l> {
    lines: &'l [usize],
    base: usize,
    out: Vec<ImportRef>,
}

impl Collector<'_> {
    fn push(&mut self, offset: u32, specifier: Option<String>, kind: ImportKind, type_only: bool) {
        let at = self.base + offset as usize;
        let line = self.lines.partition_point(|s| *s <= at);
        self.out.push(ImportRef {
            specifier,
            line: u32::try_from(line).unwrap_or(u32::MAX),
            kind,
            type_only,
        });
    }
}

/// A string literal, or a template literal with no substitution, as its text.
fn literal(e: &Expression<'_>) -> Option<String> {
    match e {
        Expression::StringLiteral(s) => Some(s.value.display().to_string()),
        Expression::TemplateLiteral(t) if t.expressions.is_empty() => {
            t.quasis[0].value.cooked.map(|c| c.display().to_string())
        }
        Expression::ParenthesizedExpression(p) => literal(&p.expression),
        _ => None,
    }
}

impl<'a> Visit<'a> for Collector<'_> {
    fn visit_import_declaration(&mut self, it: &ImportDeclaration<'a>) {
        self.push(
            it.span.start,
            Some(it.source.value.display().to_string()),
            ImportKind::Static,
            it.import_kind == ImportOrExportKind::Type,
        );
        walk::walk_import_declaration(self, it);
    }

    fn visit_export_from_declaration(&mut self, it: &ExportFromDeclaration<'a>) {
        self.push(
            it.span.start,
            Some(it.source.value.display().to_string()),
            ImportKind::ReExport,
            it.export_kind == ImportOrExportKind::Type,
        );
        walk::walk_export_from_declaration(self, it);
    }

    fn visit_export_all_declaration(&mut self, it: &ExportAllDeclaration<'a>) {
        self.push(
            it.span.start,
            Some(it.source.value.display().to_string()),
            ImportKind::ReExport,
            it.export_kind == ImportOrExportKind::Type,
        );
        walk::walk_export_all_declaration(self, it);
    }

    fn visit_import_expression(&mut self, it: &ImportExpression<'a>) {
        self.push(
            it.span.start,
            literal(&it.source),
            ImportKind::Dynamic,
            false,
        );
        walk::walk_import_expression(self, it);
    }

    fn visit_call_expression(&mut self, it: &CallExpression<'a>) {
        if let Expression::Identifier(id) = &it.callee
            && id.name == "require"
            && it.arguments.len() == 1
            && !it.optional
        {
            let spec = it.arguments[0].as_expression().and_then(literal);
            self.push(it.span.start, spec, ImportKind::Require, false);
        }
        walk::walk_call_expression(self, it);
    }

    fn visit_ts_import_equals_declaration(&mut self, it: &TSImportEqualsDeclaration<'a>) {
        if let TSModuleReference::ExternalModuleReference(r) = &it.module_reference {
            self.push(
                it.span.start,
                Some(r.expression.value.display().to_string()),
                ImportKind::Static,
                it.import_kind == ImportOrExportKind::Type,
            );
        }
        walk::walk_ts_import_equals_declaration(self, it);
    }

    fn visit_ts_import_type(&mut self, it: &TSImportType<'a>) {
        self.push(
            it.span().start,
            Some(it.source.value.display().to_string()),
            ImportKind::TypeQuery,
            true,
        );
        walk::walk_ts_import_type(self, it);
    }
}

pub(in crate::analysis) fn line_starts(source: &str) -> Vec<usize> {
    let mut starts = vec![0];
    starts.extend(source.match_indices('\n').map(|(i, _)| i + 1));
    starts
}

/// What `source` imports, or `None` when it could not be parsed at all.
#[must_use]
pub fn js_imports(path: &str, source: &str) -> Option<Vec<ImportRef>> {
    let language = Language::from_path(std::path::Path::new(path))?;
    let lines = line_starts(source);
    match language {
        Language::JavaScript | Language::Jsx | Language::TypeScript | Language::Tsx => {
            read(source, language, 0, &lines)
        }
        Language::Vue | Language::Svelte | Language::Astro => {
            let mut all = Vec::new();
            for region in script_regions(source, language) {
                let code = &source[region.start..region.end];
                if let Some(found) = read(code, region.language, region.start, &lines) {
                    all.extend(found);
                }
            }
            Some(all)
        }
        _ => None,
    }
}

fn read(code: &str, language: Language, base: usize, lines: &[usize]) -> Option<Vec<ImportRef>> {
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, code, source_type(language, false)).parse();
    if parsed.fatal_error {
        return None;
    }
    let mut collector = Collector {
        lines,
        base,
        out: Vec::new(),
    };
    collector.visit_program(&parsed.program);
    Some(collector.out)
}
