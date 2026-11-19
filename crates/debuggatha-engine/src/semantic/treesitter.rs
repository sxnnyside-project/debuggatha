//! The tree-sitter adapter. A language is a query file in `data/semantic/` that names what the
//! tree holds with the same captures everywhere: `@call` with `@callee` (or `@callee_macro`) and
//! `@args`, `@function`, `@loop` (the body that repeats), `@test`. Nesting is worked out here from
//! the spans, so no language needs code of its own.

use super::build::{Found, RawDecl, assemble, normalize};
use super::{Call, Model};
use crate::syntax::{FileSyntax, Language, QueryMatch, QuerySet, Span};

const SOURCES: [(Language, &str); 13] = [
    (Language::Rust, include_str!("../../data/semantic/rust.scm")),
    (Language::Go, include_str!("../../data/semantic/go.scm")),
    (
        Language::Python,
        include_str!("../../data/semantic/python.scm"),
    ),
    (
        Language::Kotlin,
        include_str!("../../data/semantic/kotlin.scm"),
    ),
    (Language::Php, include_str!("../../data/semantic/php.scm")),
    (Language::Java, include_str!("../../data/semantic/java.scm")),
    (
        Language::CSharp,
        include_str!("../../data/semantic/csharp.scm"),
    ),
    (Language::Ruby, include_str!("../../data/semantic/ruby.scm")),
    (
        Language::Swift,
        include_str!("../../data/semantic/swift.scm"),
    ),
    (Language::Dart, include_str!("../../data/semantic/dart.scm")),
    (
        Language::Elixir,
        include_str!("../../data/semantic/elixir.scm"),
    ),
    (Language::Cpp, include_str!("../../data/semantic/cpp.scm")),
    (
        Language::GdScript,
        include_str!("../../data/semantic/gdscript.scm"),
    ),
];

static QUERIES: QuerySet<{ SOURCES.len() }> = QuerySet::new(SOURCES);

/// Why an adapter query does not compile, if any (a test asserts there is none).
#[must_use]
pub fn compile_errors() -> Vec<String> {
    QUERIES.errors()
}

fn capture(m: &QueryMatch, name: &str) -> Option<Span> {
    m.captures.iter().find(|c| c.name == name).map(|c| c.span)
}

fn call_of(m: &QueryMatch, source: &str) -> Option<Call> {
    let args = capture(m, "args");
    let (callee_span, is_macro) = match (capture(m, "callee_macro"), capture(m, "callee")) {
        (Some(span), _) => (span, true),
        (None, Some(span)) => (span, false),
        (None, None) => {
            let call = capture(m, "call")?;
            let end = match capture(m, "callee_end") {
                Some(name) => name.end,
                None => args?.start,
            };
            (Span::new(call.start, end), false)
        }
    };
    let text = |s: Span| source.get(s.start as usize..s.end as usize);
    if is_macro && !text(args?)?.starts_with('(') {
        return None;
    }
    let mut callee = normalize(text(callee_span)?);
    if is_macro {
        callee.push('!');
    }
    let span = capture(m, "call")
        .unwrap_or_else(|| Span::new(callee_span.start, args.map_or(callee_span.end, |a| a.end)));
    Some(Call {
        span,
        callee,
        callee_span,
        args,
        arg_list: capture(m, "arg").into_iter().collect(),
        dynamic_args: Vec::new(),
        refs: Vec::new(),
        function: None,
        loop_depth: 0,
        in_test: false,
    })
}

fn collect(matches: &[QueryMatch], syntax: &FileSyntax, source: &str) -> Found {
    let mut found = Found::default();
    for m in matches {
        found.functions.extend(capture(m, "function"));
        found.loops.extend(capture(m, "loop"));
        found.tests.extend(capture(m, "test"));
        found.dynamic.extend(capture(m, "dynamic_arg"));
        found.refs.extend(capture(m, "ref"));
        found.not_refs.extend(capture(m, "not_ref"));
        found.paths.extend(capture(m, "path"));
        found.cleaned.extend(capture(m, "clean"));
        found.imports.extend(capture(m, "import"));
        found.not_imports.extend(capture(m, "not_import"));
        found.annotations.extend(capture(m, "annotation"));
        found.mentions.extend(capture(m, "mention"));
        found.dynamic_calls.extend(capture(m, "dynamic_call"));
        found.empty_handlers.extend(capture(m, "empty_handler"));
        if let Some(name) = capture(m, "decl") {
            found.decls.push(RawDecl {
                name,
                node: capture(m, "decl_node").unwrap_or(name),
                modifiers: m
                    .captures
                    .iter()
                    .filter(|c| c.name == "modifiers")
                    .map(|c| c.span)
                    .collect(),
                member: m
                    .captures
                    .iter()
                    .any(|c| c.name == "member" && c.span == name),
                wired: m
                    .captures
                    .iter()
                    .any(|c| c.name == "wired" && c.span == name),
            });
        }
        let value = capture(m, "value");
        found
            .defs
            .extend(capture(m, "def").map(|d| (d, None, value)));
        found
            .defs
            .extend(capture(m, "loop_def").map(|d| (d, capture(m, "loop"), value)));
        if let Some(call) = call_of(m, source)
            && !syntax.touches_error(call.span)
        {
            match found
                .calls
                .iter_mut()
                .find(|c| c.callee_span == call.callee_span && c.args == call.args)
            {
                Some(known) => known.arg_list.extend(call.arg_list),
                None => found.calls.push(call),
            }
        }
    }
    found
        .functions
        .sort_by_key(|s| (s.start, std::cmp::Reverse(s.end)));
    found.functions.dedup();
    found.loops.sort_by_key(|s| (s.start, s.end));
    found.loops.dedup();
    found.calls.sort_by_key(|c| (c.span.start, c.span.end));
    found
}

pub(super) fn model_of(syntax: &FileSyntax, source: &str) -> Option<Model> {
    let query = QUERIES.query_for(syntax.language)?;
    let matches = query.run(syntax, source).ok()?;
    let found = collect(&matches, syntax, source);
    let comments = syntax
        .facts
        .as_ref()
        .map(|f| f.comments.clone())
        .unwrap_or_default();
    Some(assemble(found, source, syntax.language, &comments))
}
