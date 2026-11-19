//! What both adapters share: the parts of a file they found (calls, functions, loops, names, imports,
//! declarations), and the work that turns them into one model: which function and loop a call is
//! in, which declaration a name reads, who can see a declaration.

use super::{
    Binding, Call, Declaration, Import, Mention, Model, Path, Visibility, concepts, is_wired,
    visibility,
};
use crate::syntax::{Backend, Language, Span};

pub(super) fn contains(outer: Span, inner: Span) -> bool {
    outer.start <= inner.start && inner.end <= outer.end
}

/// `a::b`, `a->b`, `a?.b` and `\a\b` are all `a.b`.
pub(super) fn normalize(text: &str) -> String {
    let joined: String = text.split_whitespace().collect();
    joined
        .replace("::", ".")
        .replace("->", ".")
        .replace("?.", ".")
        .replace('\\', ".")
        .trim_start_matches('.')
        .to_owned()
}

fn innermost(functions: &[Span], span: Span) -> Option<usize> {
    functions
        .iter()
        .enumerate()
        .filter(|(_, f)| contains(**f, span))
        .min_by_key(|(_, f)| f.end - f.start)
        .map(|(i, _)| i)
}

/// A declaration as the query gave it: its name, the node that holds it and its modifiers.
pub(super) struct RawDecl {
    pub(super) name: Span,
    pub(super) node: Span,
    pub(super) modifiers: Vec<Span>,
    pub(super) member: bool,
    pub(super) wired: bool,
}

/// A name a query found, the loop body it iterates and the value it is given.
pub(super) type Def = (Span, Option<Span>, Option<Span>);

/// What the queries found, before nesting and names are worked out.
#[derive(Default)]
pub(super) struct Found {
    pub(super) functions: Vec<Span>,
    pub(super) loops: Vec<Span>,
    pub(super) small_loops: Vec<Span>,
    /// Calls that iterate a list written out in the code.
    pub(super) literal_iterations: Vec<Span>,
    pub(super) tests: Vec<Span>,
    pub(super) dynamic: Vec<Span>,
    pub(super) defs: Vec<Def>,
    pub(super) paths: Vec<Span>,
    pub(super) cleaned: Vec<Span>,
    pub(super) imports: Vec<Span>,
    pub(super) not_imports: Vec<Span>,
    pub(super) decls: Vec<RawDecl>,
    /// Names an `export` carries: public, where a language without modifiers says so.
    pub(super) exported: Vec<Span>,
    pub(super) annotations: Vec<Span>,
    pub(super) mentions: Vec<Span>,
    pub(super) dynamic_calls: Vec<Span>,
    pub(super) empty_handlers: Vec<Span>,
    pub(super) refs: Vec<Span>,
    pub(super) not_refs: Vec<Span>,
    pub(super) calls: Vec<Call>,
}

fn bindings_of(found: &Found, source: &str) -> Vec<Binding> {
    let mut defs = found.defs.clone();
    // A loop header's name must win over the plain declaration of the same node.
    defs.sort_by_key(|(span, loop_body, value)| {
        (span.start, span.end, loop_body.is_none(), value.is_none())
    });
    defs.dedup_by_key(|(span, ..)| *span);
    defs.iter()
        .filter_map(|(span, loop_body, value)| {
            Some(Binding {
                name: source
                    .get(span.start as usize..span.end as usize)?
                    .to_owned(),
                span: *span,
                function: innermost(&found.functions, *span),
                loop_body: *loop_body,
                value: *value,
            })
        })
        .collect()
}

/// Each name read, and every binding it may read: the earlier ones of the same name in the
/// function or in one that contains it. Which one reaches is not told apart, nor are blocks.
fn resolve(found: &Found, bindings: &[Binding], source: &str) -> Vec<(Span, usize)> {
    found
        .refs
        .iter()
        .filter(|r| !found.not_refs.contains(r) && !bindings.iter().any(|b| b.span == **r))
        .flat_map(|r| {
            let name = source
                .get(r.start as usize..r.end as usize)
                .unwrap_or_default();
            bindings
                .iter()
                .enumerate()
                .filter(move |(_, b)| {
                    b.name == name
                        && b.span.start <= r.start
                        && b.function.is_none_or(|f| contains(found.functions[f], *r))
                })
                .map(move |(i, _)| (*r, i))
        })
        .collect()
}

fn imports_of(spans: &[Span], not: &[Span], source: &str) -> Vec<Import> {
    let mut spans: Vec<Span> = spans.iter().filter(|s| !not.contains(s)).copied().collect();
    spans.sort_by_key(|s| (s.start, s.end));
    spans.dedup();
    spans
        .into_iter()
        .filter_map(|span| {
            let text = source.get(span.start as usize..span.end as usize)?;
            let before = source.get(..span.start as usize)?;
            Some(Import {
                specifier: text.trim_matches(['"', '\'', '`']).to_owned(),
                line: u32::try_from(before.matches('\n').count() + 1).unwrap_or(u32::MAX),
            })
        })
        .collect()
}

fn text_of(source: &str, span: Span) -> Option<&str> {
    source.get(span.start as usize..span.end as usize)
}

fn declarations_of(found: &Found, source: &str, language: Language) -> Vec<Declaration> {
    let mut raw: Vec<&RawDecl> = found.decls.iter().collect();
    // The widest node holds the decorators of a decorated definition.
    raw.sort_by_key(|d| {
        (
            d.name.start,
            d.name.end,
            std::cmp::Reverse(d.node.end - d.node.start),
        )
    });
    raw.dedup_by_key(|d| d.name);
    raw.into_iter()
        .filter_map(|d| {
            let name = text_of(source, d.name)?;
            let modifiers = d
                .modifiers
                .iter()
                .filter_map(|m| text_of(source, *m))
                .collect::<Vec<_>>()
                .join(" ");
            let before = source.get(..d.name.start as usize)?;
            Some(Declaration {
                name: name.to_owned(),
                span: d.name,
                line: u32::try_from(before.matches('\n').count() + 1).unwrap_or(u32::MAX),
                visibility: if matches!(language.backend(), Backend::Oxc | Backend::Sfc) {
                    if found.exported.contains(&d.name) {
                        Visibility::Public
                    } else {
                        Visibility::Private
                    }
                } else {
                    visibility(language, name, &modifiers, d.member)
                },
                member: d.member,
                decorated: d.wired
                    || is_wired(language, &modifiers)
                    || found
                        .annotations
                        .iter()
                        .any(|a| d.node.start <= a.start && a.end <= d.name.start),
            })
        })
        .collect()
}

fn mentions_of(found: &Found, source: &str) -> Vec<Mention> {
    let mut spans: Vec<Span> = found
        .mentions
        .iter()
        .filter(|m| !found.decls.iter().any(|d| d.name == **m))
        .copied()
        .collect();
    spans.sort_by_key(|s| (s.start, s.end));
    spans.dedup();
    spans
        .into_iter()
        .filter_map(|span| {
            Some(Mention {
                name: text_of(source, span)?.to_owned(),
                span,
            })
        })
        .collect()
}

/// Member chains and every name read, as written.
fn paths_of(found: &Found, source: &str) -> Vec<Path> {
    let mut spans: Vec<Span> = found
        .paths
        .iter()
        .chain(found.refs.iter().filter(|r| !found.not_refs.contains(r)))
        .copied()
        .collect();
    spans.sort_by_key(|s| (s.start, s.end));
    spans.dedup();
    spans
        .into_iter()
        .filter_map(|span| {
            Some(Path {
                span,
                text: normalize(source.get(span.start as usize..span.end as usize)?),
            })
        })
        .collect()
}

/// The functions handed to a call the `iteration-call` concept binds: each runs once per element,
/// so it is a loop body. Only those that begin after the callee and are not inside another one.
fn callbacks(calls: &[Call], functions: &[Span], language: Language) -> Vec<(Span, Span)> {
    let table = concepts();
    calls
        .iter()
        .filter(|c| table.is("iteration-call", language, &c.callee))
        .flat_map(|call| {
            let inside: Vec<Span> = functions
                .iter()
                .filter(|f| f.start >= call.callee_span.end && contains(call.span, **f))
                .copied()
                .collect();
            inside
                .iter()
                .filter(|f| !inside.iter().any(|o| o != *f && contains(*o, **f)))
                .map(|f| (*f, call.span))
                .collect::<Vec<_>>()
        })
        .collect()
}

/// The model of what an adapter found. `comments` are the spans no argument may overlap.
pub(super) fn assemble(
    mut found: Found,
    source: &str,
    language: Language,
    comments: &[Span],
) -> Model {
    let bindings = bindings_of(&found, source);
    let reads = resolve(&found, &bindings, source);
    let paths = paths_of(&found, source);
    let mut calls = std::mem::take(&mut found.calls);
    for (body, call) in callbacks(&calls, &found.functions, language) {
        found.loops.push(body);
        if found.literal_iterations.contains(&call) {
            found.small_loops.push(body);
        }
    }
    found.loops.sort_by_key(|s| (s.start, s.end));
    found.loops.dedup();
    for call in &mut calls {
        let around = innermost(&found.functions, call.span);
        call.function = around;
        let floor = around.map_or(0, |i| found.functions[i].start);
        call.loop_depth = found
            .loops
            .iter()
            .filter(|l| l.start >= floor && contains(**l, call.span))
            .count()
            .try_into()
            .unwrap_or(u32::MAX);
        call.dynamic_args = found
            .dynamic
            .iter()
            .filter(|d| call.args.is_some_and(|a| contains(a, **d)))
            .copied()
            .collect();
        call.refs = reads
            .iter()
            .filter(|(r, _)| call.args.is_some_and(|a| contains(a, *r)))
            .map(|(_, i)| *i)
            .collect();
        call.arg_list.sort_by_key(|a| a.start);
        call.arg_list.dedup();
        call.arg_list
            .retain(|a| !comments.iter().any(|c| c.overlaps(*a)));
        call.refs.sort_unstable();
        call.refs.dedup();
        call.in_test = found.tests.iter().any(|t| contains(*t, call.span));
    }
    let declarations = declarations_of(&found, source, language);
    let mentions = mentions_of(&found, source);
    let mut dynamic_calls = found.dynamic_calls.clone();
    dynamic_calls.extend(
        calls
            .iter()
            .filter(|c| concepts().is("dynamic-dispatch", language, &c.callee))
            .map(|c| c.span),
    );
    let imports = imports_of(&found.imports, &found.not_imports, source);
    Model {
        calls,
        functions: found.functions,
        loops: found.loops,
        small_loops: found.small_loops,
        bindings,
        reads,
        paths,
        cleaned: found.cleaned,
        imports,
        declarations,
        mentions,
        dynamic_calls,
        empty_handlers: found.empty_handlers,
    }
}
