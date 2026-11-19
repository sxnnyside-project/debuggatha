//! Group A: the syntax rules of the JavaScript/TypeScript family, written over oxc's tree and
//! symbols (ADR-0006, decision 4). One parse feeds all ten rules.
//!
//! A hit carries the file extension, confidence, columns and the excerpt of its whole line, with one
//! hit per line and rule, so ledger identities hold. The tree gives exactness: no match inside a
//! string, a comment or JSX text, destructured `var`, a local `setTimeout`, `x!` that is only a
//! definite assignment. Differences from the TypeScript engine are listed in `docs/PARITY.md`.

use oxc_allocator::Allocator;
use oxc_ast::ast::{
    AssignmentExpression, AssignmentOperator, AssignmentTarget, BinaryExpression, BinaryOperator,
    CallExpression, Expression, IdentifierName, IdentifierReference, JSXIdentifier, NewExpression,
    TSAnyKeyword, TSExternalModuleDeclaration, TSGlobalDeclaration, TSNamespaceDeclaration,
    TSNonNullExpression, VariableDeclaration, VariableDeclarationKind,
};
use oxc_ast_visit::{Visit, walk};
use oxc_parser::Parser;
use oxc_semantic::{Scoping, SemanticBuilder};
use oxc_span::GetSpan;

use crate::detect::excerpt::excerpt_of;
use crate::detect::hit::{Hit, HitEdit};
use crate::detect::input::DetectorInput;
use crate::detect::runner::Registry;
use crate::scan_scope::is_test_path;
use crate::syntax::oxc::source_type;
use crate::syntax::{Language, Span};
use debuggatha_core::review_engine::{Confidence, EditSafety, LineRange};

/// The rules this group answers for.
pub const RULES: &[&str] = &[
    "no-var",
    "no-explicit-any",
    "strict-null-checks",
    "eqeqeq",
    "no-eval",
    "no-implied-eval",
    "no-extend-native",
    "prefer-object-spread",
    "prefer-template-literals",
    "no-dangerously-set-inner-html",
];

/// Registers the group.
pub fn register(registry: &mut Registry) {
    registry.register_group(RULES, detect);
}

const NATIVES: &[&str] = &["Object", "Array", "String", "Number", "Boolean", "Function"];

pub(in crate::detect) fn extension(file: &str) -> &str {
    file.rsplit_once('.').map_or("", |(_, e)| e)
}

/// `.ts .tsx .js .jsx .mjs .cjs`, the files the TypeScript rules guard on.
pub(in crate::detect) fn is_js_like(file: &str) -> bool {
    matches!(extension(file), "ts" | "tsx" | "js" | "jsx" | "mjs" | "cjs")
}
fn is_ts_like(file: &str) -> bool {
    matches!(extension(file), "ts" | "tsx")
}
fn is_jsx_like(file: &str) -> bool {
    matches!(extension(file), "tsx" | "jsx")
}

/// Byte offsets of line starts, to turn an offset into a line and a UTF-16 column.
pub(in crate::detect) struct Lines<'s> {
    source: &'s str,
    starts: Vec<usize>,
}

impl<'s> Lines<'s> {
    pub(in crate::detect) fn new(source: &'s str) -> Self {
        let mut starts = vec![0];
        starts.extend(source.match_indices('\n').map(|(i, _)| i + 1));
        Self { source, starts }
    }

    /// 1-based line of a byte offset.
    pub(in crate::detect) fn line(&self, offset: usize) -> usize {
        self.starts.partition_point(|s| *s <= offset)
    }

    /// 1-based UTF-16 column of a byte offset.
    pub(in crate::detect) fn column(&self, offset: usize) -> u32 {
        let start = self.starts[self.line(offset) - 1];
        let n = self.source[start..offset].encode_utf16().count();
        u32::try_from(n).unwrap_or(u32::MAX) + 1
    }

    pub(in crate::detect) fn raw_line(&self, line: usize) -> &'s str {
        let start = self.starts[line - 1];
        let end = self.starts.get(line).map_or(self.source.len(), |e| e - 1);
        &self.source[start..end]
    }
}

/// A hit for the source range `start..end`, or `None` when the parser could not read that region
/// (nothing is concluded about it). Columns are UTF-16 and the excerpt is the whole line.
pub(in crate::detect) fn make_hit(
    input: &DetectorInput<'_>,
    lines: &Lines<'_>,
    start: usize,
    end: usize,
    confidence: Confidence,
    edit: Option<(usize, usize, &str, EditSafety)>,
) -> Option<Hit> {
    if input
        .syntax
        .is_some_and(|s| s.touches_error(span(start, end)))
    {
        return None;
    }
    let source = lines.source;
    let line = lines.line(start);
    let col = lines.column(start);
    let width = u32::try_from(
        source[start..end.max(start)]
            .split('\n')
            .next()
            .map_or(0, |t| t.encode_utf16().count()),
    )
    .unwrap_or(0);
    let edit = edit.map(|(from, to, replacement, safety)| HitEdit {
        start_column: lines.column(from),
        end_column: lines.column(from)
            + u32::try_from(source[from..to].encode_utf16().count()).unwrap_or(0),
        replacement: replacement.to_owned(),
        safety,
    });
    Some(Hit {
        line: u32::try_from(line).unwrap_or(u32::MAX),
        columns: LineRange {
            start: col,
            end: col + width,
        },
        excerpt: excerpt_of(lines.raw_line(line)),
        confidence,
        severity: None,
        edit,
    })
}

struct Collector<'a, 's> {
    source: &'s str,
    /// `source` with the bytes of every comment replaced by spaces: same offsets, so a scan
    /// between two tokens never reads a comment.
    code: String,
    lines: Lines<'s>,
    scoping: &'a Scoping,
    file: &'s str,
    input: &'a DetectorInput<'s>,
    tests: bool,
    ambient: u32,
    /// 0 for a hit a text pattern also finds, 1 for one only the tree finds: on a line with both,
    /// the first kind is reported, so its columns stay stable.
    rank: u8,
    hits: Vec<(&'static str, Hit, usize, u8)>,
}

impl Collector<'_, '_> {
    fn global(&self, id: &IdentifierReference<'_>) -> bool {
        id.reference_id
            .get()
            .is_some_and(|r| self.scoping.get_reference(r).symbol_id().is_none())
    }

    fn push(
        &mut self,
        rule: &'static str,
        start: usize,
        end: usize,
        confidence: Confidence,
        edit: Option<(usize, usize, &str, EditSafety)>,
    ) {
        if let Some(hit) = make_hit(self.input, &self.lines, start, end, confidence, edit) {
            self.hits.push((rule, hit, start, self.rank));
        }
        self.rank = 0;
    }

    /// The offset just past the first `(` after `from`, if only spaces lie between.
    fn after_paren(&self, from: usize) -> Option<usize> {
        let rest = &self.code[from..];
        let trimmed = rest.trim_start();
        trimmed
            .starts_with('(')
            .then(|| from + (rest.len() - trimmed.len()) + 1)
    }

    /// `"text" + name` and `name + "text"`, which a template literal reads better. In `a + "x" + b`
    /// the string is the last thing the left side ends with.
    fn concatenation(&mut self, it: &BinaryExpression<'_>) {
        let mut leaf = &it.left;
        while let Expression::BinaryExpression(inner) = leaf
            && inner.operator == BinaryOperator::Addition
        {
            leaf = &inner.right;
        }
        let right_text = is_stringish(&it.right);
        let (rstart, _) = range(&it.right);
        if is_stringish(leaf) && !right_text {
            // A string, then a name: a letter, `_` or `$` must follow.
            let first = self.source[rstart..].chars().next();
            if first.is_some_and(|c| c.is_ascii_alphabetic() || c == '_' || c == '$') {
                let (start, _) = range(leaf);
                self.push(
                    "prefer-template-literals",
                    start,
                    rstart + 1,
                    Confidence::Low,
                    None,
                );
            }
        } else if right_text && !is_stringish(leaf) {
            // A name, then a string: the left side must end in a run of name characters that holds
            // a letter, `_` or `$` (not a call, an index or a number).
            let (_, lend) = range(&it.left);
            let run_len = self.source[..lend]
                .chars()
                .rev()
                .take_while(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '$' | '.'))
                .count();
            let run = &self.source[lend - run_len..lend];
            let start = run.char_indices().find_map(|(i, c)| {
                let begins = c.is_ascii_alphabetic() || c == '_' || c == '$';
                (begins
                    && run[i + 1..]
                        .chars()
                        .all(|n| n.is_ascii_alphanumeric() || n == '_' || n == '.'))
                .then_some(lend - run_len + i)
            });
            if let Some(start) = start {
                // The old match ended on the opening quote.
                self.push(
                    "prefer-template-literals",
                    start,
                    rstart + 1,
                    Confidence::Low,
                    None,
                );
            }
        }
    }

    /// The offset of `op` in the code between two tokens.
    fn find_between(&self, from: usize, to: usize, op: &str) -> Option<usize> {
        self.code.get(from..to)?.find(op).map(|i| from + i)
    }
}

pub(in crate::detect) fn span(start: usize, end: usize) -> Span {
    Span::new(
        u32::try_from(start).unwrap_or(u32::MAX),
        u32::try_from(end).unwrap_or(u32::MAX),
    )
}

pub(in crate::detect) fn range<T: GetSpan>(node: &T) -> (usize, usize) {
    let s = node.span();
    (s.start as usize, s.end as usize)
}

pub(in crate::detect) fn peel<'e, 'a>(mut e: &'e Expression<'a>) -> &'e Expression<'a> {
    while let Expression::ParenthesizedExpression(p) = e {
        e = &p.expression;
    }
    e
}

/// The symbol an identifier refers to, if it refers to one the file declares.
pub(in crate::detect) fn symbol_index(
    semantic: &oxc_semantic::Semantic<'_>,
    id: &oxc_ast::ast::IdentifierReference<'_>,
) -> Option<usize> {
    semantic
        .scoping()
        .get_reference(id.reference_id())
        .symbol_id()
        .map(oxc_semantic::SymbolId::index)
}

pub(in crate::detect) fn is_stringish(e: &Expression<'_>) -> bool {
    matches!(
        e,
        Expression::StringLiteral(_) | Expression::TemplateLiteral(_)
    )
}

impl<'a> Visit<'a> for Collector<'a, '_> {
    fn visit_ts_global_declaration(&mut self, it: &TSGlobalDeclaration<'a>) {
        self.ambient += 1;
        walk::walk_ts_global_declaration(self, it);
        self.ambient -= 1;
    }

    fn visit_ts_namespace_declaration(&mut self, it: &TSNamespaceDeclaration<'a>) {
        let ambient = u32::from(it.declare);
        self.ambient += ambient;
        walk::walk_ts_namespace_declaration(self, it);
        self.ambient -= ambient;
    }

    fn visit_ts_external_module_declaration(&mut self, it: &TSExternalModuleDeclaration<'a>) {
        let ambient = u32::from(it.declare);
        self.ambient += ambient;
        walk::walk_ts_external_module_declaration(self, it);
        self.ambient -= ambient;
    }

    fn visit_variable_declaration(&mut self, it: &VariableDeclaration<'a>) {
        // `declare var` and `var` inside `declare global` describe a global; they are not code.
        if it.kind == VariableDeclarationKind::Var && !it.declare && self.ambient == 0 {
            let (start, _) = range(it);
            let after = start + 3;
            let rest = &self.source[after..];
            let name = rest.trim_start();
            // The old match was `var`, spaces and the first character of the name.
            let end = match name.chars().next() {
                Some(c)
                    if rest.len() > name.len()
                        && (c.is_ascii_alphabetic() || c == '_' || c == '$') =>
                {
                    after + (rest.len() - name.len()) + c.len_utf8()
                }
                _ => after,
            };
            if is_js_like(self.file) {
                self.push(
                    "no-var",
                    start,
                    end,
                    Confidence::High,
                    Some((start, after, "let", EditSafety::Safe)),
                );
            }
        }
        walk::walk_variable_declaration(self, it);
    }

    fn visit_ts_any_keyword(&mut self, it: &TSAnyKeyword) {
        if is_ts_like(self.file) {
            let (start, end) = (it.span.start as usize, it.span.end as usize);
            let before = self.source[..start].trim_end();
            // Columns of the old match where there was one: `: any`, `as any`, `<any>`.
            let from = if before.ends_with(':') {
                before.len() - 1
            } else if before.ends_with("as")
                && !before[..before.len() - 2]
                    .chars()
                    .next_back()
                    .is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '$')
            {
                before.len() - 2
            } else if before.ends_with('<') && self.source[end..].starts_with('>') {
                before.len() - 1
            } else {
                start
            };
            let to = if self.source[end..].starts_with('>')
                && from < start
                && self.source[from..].starts_with('<')
            {
                end + 1
            } else {
                end
            };
            self.rank = u8::from(from == start);
            // `unknown` is the right direction, but callers may rely on `any`: review the result.
            self.push(
                "no-explicit-any",
                from,
                to,
                Confidence::High,
                Some((start, end, "unknown", EditSafety::Review)),
            );
        }
    }

    fn visit_ts_non_null_expression(&mut self, it: &TSNonNullExpression<'a>) {
        if is_ts_like(self.file) {
            let bang = it.span.end as usize - 1;
            let prev = self.source[..bang]
                .char_indices()
                .next_back()
                .map_or(bang, |(i, _)| i);
            self.push(
                "strict-null-checks",
                prev,
                bang + 1,
                Confidence::Medium,
                None,
            );
        }
        walk::walk_ts_non_null_expression(self, it);
    }

    fn visit_binary_expression(&mut self, it: &BinaryExpression<'a>) {
        let (l, r) = (peel(&it.left), peel(&it.right));
        match it.operator {
            BinaryOperator::Equality | BinaryOperator::Inequality if is_js_like(self.file) => {
                // `x == null` is the accepted way to test null or undefined at once.
                let null = matches!(l, Expression::NullLiteral(_))
                    || matches!(r, Expression::NullLiteral(_));
                if !null {
                    let op = if it.operator == BinaryOperator::Equality {
                        "=="
                    } else {
                        "!="
                    };
                    if let Some(at) = self.find_between(
                        it.left.span().end as usize,
                        it.right.span().start as usize,
                        op,
                    ) {
                        let replacement = if op == "==" { "===" } else { "!==" };
                        self.push(
                            "eqeqeq",
                            at,
                            at + 2,
                            Confidence::High,
                            Some((at, at + 2, replacement, EditSafety::Review)),
                        );
                    }
                }
            }
            BinaryOperator::Addition if is_js_like(self.file) => self.concatenation(it),
            _ => {}
        }
        walk::walk_binary_expression(self, it);
    }

    fn visit_call_expression(&mut self, it: &CallExpression<'a>) {
        if let Expression::Identifier(id) = &it.callee
            && is_js_like(self.file)
            && self.global(id)
        {
            let (start, end) = range(&**id);
            match id.name.as_str() {
                "eval" => {
                    if let Some(open) = self.after_paren(end) {
                        self.push("no-eval", start, open, Confidence::High, None);
                    }
                }
                "setTimeout" | "setInterval" => {
                    if let Some(first) = it.arguments.first().and_then(|a| a.as_expression())
                        && is_stringish(first)
                    {
                        // The old match ended on the opening quote of the string.
                        self.push(
                            "no-eval",
                            start,
                            first.span().start as usize + 1,
                            Confidence::High,
                            None,
                        );
                    }
                }
                _ => {}
            }
        }
        if let Expression::StaticMemberExpression(m) = &it.callee
            && is_js_like(self.file)
            && m.property.name == "assign"
            && let Expression::Identifier(obj) = &m.object
            && obj.name == "Object"
            && self.global(obj)
            && let Some(first) = it.arguments.first().and_then(|a| a.as_expression())
            && let Expression::ObjectExpression(o) = first
            && o.properties.is_empty()
        {
            let (start, _) = range(&**m);
            let (_, end) = range(&**o);
            self.push("prefer-object-spread", start, end, Confidence::Medium, None);
        }
        walk::walk_call_expression(self, it);
    }

    fn visit_new_expression(&mut self, it: &NewExpression<'a>) {
        if let Expression::Identifier(id) = &it.callee
            && id.name == "Function"
            && is_js_like(self.file)
            && !self.tests
            && self.global(id)
            && let Some(open) = self.after_paren(id.span.end as usize)
        {
            self.push(
                "no-implied-eval",
                it.span.start as usize,
                open,
                Confidence::High,
                None,
            );
        }
        walk::walk_new_expression(self, it);
    }

    fn visit_assignment_expression(&mut self, it: &AssignmentExpression<'a>) {
        if it.operator == AssignmentOperator::Assign
            && is_js_like(self.file)
            && let AssignmentTarget::StaticMemberExpression(m) = &it.left
            && let Expression::StaticMemberExpression(proto) = &m.object
            && proto.property.name == "prototype"
            && let Expression::Identifier(native) = &proto.object
            && NATIVES.contains(&native.name.as_str())
            && self.global(native)
        {
            let (start, lend) = range(&**m);
            if let Some(eq) = self.find_between(lend, it.right.span().start as usize, "=") {
                self.push("no-extend-native", start, eq + 1, Confidence::High, None);
            }
        }
        walk::walk_assignment_expression(self, it);
    }

    fn visit_jsx_identifier(&mut self, it: &JSXIdentifier<'a>) {
        if it.name == "dangerouslySetInnerHTML" && is_jsx_like(self.file) {
            let (s, e) = range(it);
            self.push(
                "no-dangerously-set-inner-html",
                s,
                e,
                Confidence::High,
                None,
            );
        }
    }

    fn visit_identifier_name(&mut self, it: &IdentifierName<'a>) {
        if it.name == "dangerouslySetInnerHTML" && is_jsx_like(self.file) {
            let (s, e) = range(it);
            self.push(
                "no-dangerously-set-inner-html",
                s,
                e,
                Confidence::High,
                None,
            );
        }
    }
}

/// Blank the bytes of every comment, keeping every offset.
pub(in crate::detect) fn blank_comments(source: &str, comments: &[Span]) -> String {
    let mut blank = vec![false; source.len()];
    for c in comments {
        let end = (c.end as usize).min(blank.len());
        blank[(c.start as usize).min(end)..end].fill(true);
    }
    source
        .char_indices()
        .map(|(at, ch)| {
            if blank[at] {
                " ".repeat(ch.len_utf8())
            } else {
                ch.to_string()
            }
        })
        .collect()
}

#[must_use]
pub fn detect(input: &DetectorInput<'_>) -> Vec<(&'static str, Hit)> {
    let Some(syntax) = input.syntax else {
        return Vec::new();
    };
    let language = syntax.language;
    if !matches!(
        language,
        Language::JavaScript | Language::Jsx | Language::TypeScript | Language::Tsx
    ) {
        return Vec::new();
    }

    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, input.content, source_type(language, false)).parse();
    if parsed.fatal_error {
        return Vec::new();
    }
    let semantic = SemanticBuilder::new().build(&parsed.program).semantic;

    let comments: Vec<Span> = syntax
        .facts
        .as_ref()
        .map(|f| f.comments.clone())
        .unwrap_or_default();
    let mut collector = Collector {
        source: input.content,
        code: blank_comments(input.content, &comments),
        lines: Lines::new(input.content),
        scoping: semantic.scoping(),
        file: input.file,
        input,
        tests: is_test_path(input.file),
        ambient: 0,
        rank: 0,
        hits: Vec::new(),
    };
    collector.visit_program(&parsed.program);

    // Source order, then one hit per line and rule: the rule of the TypeScript detectors, kept so
    // that a finding's line (and so its ledger identity) does not move.
    let mut hits = collector.hits;
    // Equal starts: the shorter match first, as the leftmost-shortest regex match did.
    hits.sort_by_key(|(_, h, start, rank)| (h.line, *rank, *start, h.columns.end));
    let mut seen = std::collections::BTreeSet::new();
    hits.into_iter()
        .filter(|(rule, h, _, _)| seen.insert((*rule, h.line)))
        .map(|(rule, h, _, _)| (rule, h))
        .collect()
}
