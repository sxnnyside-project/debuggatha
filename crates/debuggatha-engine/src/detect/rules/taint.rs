//! `a03-injection` and `validate-input`, from one small taint analysis over the request-shaped
//! sources of JavaScript and TypeScript.
//!
//! **Sources**: `req.body|query|params|headers|cookies` where `req` (or `request`) is the first
//! parameter of a function, Koa's `ctx.*`, `await req.json()` and Hono's `c.req.*`. A destructuring
//! taints every name it binds. **Propagation**: inside one file, flow-insensitive, to a fixed point,
//! through bindings, assignments, templates, `+`, `||`/`??`/`?:`, member access, literals, string
//! methods and iteration callbacks. Numeric coercion, anything named like a cleaner and a schema
//! call stop it, and so does a call to a function the file does not define (its result is clean).
//!
//! **`a03-injection` sinks**: SQL text built from tainted data (`query`, `execute`, `raw` on a
//! data-layer or driver receiver, `*Raw`, a template in `where`), the command of `child_process`
//! calls, a `$where` with tainted code and a request object passed whole as a Mongo filter. A
//! tagged `sql` template and a parameterized string are safe. **`validate-input`**: a handler that
//! reads `req.body` or `req.query` and calls no validator, reported high when the file registers it
//! with no validator in the chain and medium when a middleware elsewhere may validate it.
//!
//! **Confidence**: high when the value reaches the sink through `const` bindings only, low (at
//! `medium` severity) when it crosses a variable assigned more than once.

mod ast;
mod model;
mod pass;
mod sinks;
mod validators;
mod vocabulary;

use self::vocabulary::VALIDATE;
use oxc_ast_visit::Visit;

use self::model::{Prop, State};
use self::pass::Pass;
use crate::detect::hit::Hit;
use crate::detect::input::DetectorInput;
use crate::detect::rules::calls::one_per_line;
use crate::detect::rules::js::{Lines, make_hit};
use crate::detect::runner::Registry;
use crate::scan_scope::is_test_path;
use crate::syntax::Language;
use crate::syntax::oxc::source_type;
use debuggatha_core::review_engine::Confidence;
use oxc_allocator::Allocator;
use oxc_parser::Parser;
use oxc_semantic::SemanticBuilder;

/// The rules this detector answers for.
pub const RULES: &[&str] = &["a03-injection", "validate-input"];

/// Registers the detector.
pub fn register(registry: &mut Registry) {
    registry.register_group(RULES, detect);
}

/// Runs the detector over one file.
#[must_use]
pub fn detect(input: &DetectorInput<'_>) -> Vec<(&'static str, Hit)> {
    let Some(syntax) = input.syntax else {
        return Vec::new();
    };
    // A request is named `req`, `request`, `ctx`, `context` or `c`: a file with none of the first
    // four cannot hold one (a speed check, not a detection).
    if is_test_path(input.file)
        || !matches!(
            syntax.language,
            Language::JavaScript | Language::Jsx | Language::TypeScript | Language::Tsx
        )
        || !["req", "ctx", "context"]
            .iter()
            .any(|n| input.content.contains(n))
    {
        return Vec::new();
    }
    let allocator = Allocator::default();
    let parsed = Parser::new(
        &allocator,
        input.content,
        source_type(syntax.language, false),
    )
    .parse();
    if parsed.fatal_error {
        return Vec::new();
    }
    let semantic = SemanticBuilder::new()
        .with_build_nodes(true)
        .build(&parsed.program)
        .semantic;
    let mut state = State::default();
    let run = |state: &mut State, collect: bool, report: bool| {
        Pass {
            semantic: &semantic,
            input,
            lines: Lines::new(input.content),
            state,
            collect,
            report,
        }
        .visit_program(&parsed.program);
    };
    // Taint only grows, so the passes end; the cap is for a file built to make them long.
    for pass in 0..16 {
        state.changed = false;
        run(&mut state, pass == 0, false);
        if !state.changed {
            break;
        }
    }
    run(&mut state, false, true);
    handler_hits(input, &mut state);
    one_per_line(std::mem::take(&mut state.hits))
}

/// The `validate-input` hits: handlers that read the body or the query and validate nothing.
fn handler_hits(input: &DetectorInput<'_>, state: &mut State) {
    let lines = Lines::new(input.content);
    let mut found = Vec::new();
    for handler in &state.handlers {
        if handler.validated || handler.upstream || state.global_validation {
            continue;
        }
        let confidence = if handler.registered {
            Confidence::High
        } else {
            Confidence::Medium
        };
        for prop in [Prop::Body, Prop::Query] {
            let first = handler
                .reads
                .iter()
                .filter(|(p, start, end)| {
                    *p == prop
                        && !state
                            .logging
                            .iter()
                            .any(|(from, to)| *from <= *start && *end <= *to)
                })
                .min_by_key(|(_, start, _)| *start);
            if let Some((_, start, end)) = first
                && let Some(hit) = make_hit(input, &lines, *start, *end, confidence, None)
            {
                found.push((VALIDATE, hit, *start));
            }
        }
    }
    state.hits.extend(found);
}
