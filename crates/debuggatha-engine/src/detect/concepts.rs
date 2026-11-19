//! Rules written once over concepts (`data/concepts.toml`) and read from the semantic model, so
//! they run in every language whose adapter and table bind the concept.

use std::path::Path;

use super::calls::one_per_line;
use super::hit::Hit;
use super::input::DetectorInput;
use super::js::{Lines, make_hit};
use super::runner::Registry;
use crate::scan_scope::is_test_path;
use crate::semantic::{Call, Certainty, Flow, Model, Status, concepts, model_of};
use crate::syntax::{Backend, Language, Span};
use debuggatha_core::review_engine::{Confidence, Severity};

/// What else a call bound to the concept must do to be reported.
#[derive(Clone, Copy)]
enum When {
    Always,
    /// Its first argument is built from interpolation or concatenation.
    CommandIsBuilt,
    /// It is in a loop and reads a name the loop introduces.
    ReadsIteration,
    /// Untrusted input reaches the argument the concept is about.
    ReceivesInput,
    /// The call does not name a keyword argument (`requests.get` with no `timeout`).
    WithoutKeyword(&'static str),
    /// It takes no option beyond the data (an option such as `allowed_classes` limits the call).
    WithoutOptions,
}

struct Bound {
    rule: &'static str,
    concept: &'static str,
    when: When,
    confidence: Confidence,
    /// A guess from names must not fail a run the way a proven finding does.
    severity: Option<Severity>,
    javascript: bool,
}

const BOUND: [Bound; 43] = [
    Bound {
        rule: "no-eval",
        concept: "dynamic-eval",
        when: When::Always,
        confidence: Confidence::High,
        severity: None,
        javascript: false,
    },
    Bound {
        rule: "no-panic",
        concept: "panic",
        when: When::Always,
        confidence: Confidence::High,
        severity: None,
        javascript: false,
    },
    Bound {
        rule: "no-shell-interpolation",
        concept: "shell-sink",
        when: When::CommandIsBuilt,
        confidence: Confidence::Medium,
        severity: None,
        javascript: false,
    },
    Bound {
        rule: "n-plus-one-queries",
        concept: "data-query",
        when: When::ReadsIteration,
        confidence: Confidence::Low,
        severity: Some(Severity::Medium),
        javascript: true,
    },
    Bound {
        rule: "a03-injection",
        concept: "sql-sink",
        when: When::ReceivesInput,
        confidence: Confidence::High,
        severity: None,
        javascript: false,
    },
    Bound {
        rule: "a03-injection",
        concept: "shell-sink",
        when: When::ReceivesInput,
        confidence: Confidence::High,
        severity: None,
        javascript: false,
    },
    Bound {
        rule: "no-unsafe-deserialization",
        concept: "unsafe-deserialization",
        when: When::WithoutOptions,
        confidence: Confidence::Medium,
        severity: None,
        javascript: false,
    },
    Bound {
        rule: "no-weak-hash",
        concept: "weak-hash",
        when: When::Always,
        confidence: Confidence::Low,
        severity: Some(Severity::Medium),
        javascript: false,
    },
    Bound {
        rule: "no-print-stack-trace",
        concept: "stack-trace-print",
        when: When::Always,
        confidence: Confidence::High,
        severity: None,
        javascript: false,
    },
    Bound {
        rule: "no-fatal-error",
        concept: "crash-call",
        when: When::Always,
        confidence: Confidence::High,
        severity: None,
        javascript: false,
    },
    Bound {
        rule: "no-raw-markup",
        concept: "raw-markup",
        when: When::Always,
        confidence: Confidence::Medium,
        severity: None,
        javascript: false,
    },
    Bound {
        rule: "no-permit-all",
        concept: "permit-all",
        when: When::Always,
        confidence: Confidence::High,
        severity: None,
        javascript: false,
    },
    Bound {
        rule: "no-atom-from-input",
        concept: "atom-from-string",
        when: When::ReceivesInput,
        confidence: Confidence::High,
        severity: None,
        javascript: false,
    },
    Bound {
        rule: "no-unbounded-c-string",
        concept: "unbounded-c-string",
        when: When::Always,
        confidence: Confidence::High,
        severity: None,
        javascript: false,
    },
    Bound {
        rule: "no-debug-output",
        concept: "debug-output",
        when: When::Always,
        confidence: Confidence::High,
        severity: None,
        javascript: false,
    },
    Bound {
        rule: "no-weak-random",
        concept: "weak-random",
        when: When::Always,
        confidence: Confidence::Low,
        severity: Some(Severity::Medium),
        javascript: true,
    },
    Bound {
        rule: "a10-ssrf",
        concept: "http-client",
        when: When::ReceivesInput,
        confidence: Confidence::High,
        severity: None,
        javascript: true,
    },
    Bound {
        rule: "no-path-traversal",
        concept: "file-open",
        when: When::ReceivesInput,
        confidence: Confidence::High,
        severity: None,
        javascript: true,
    },
    Bound {
        rule: "no-env-outside-config",
        concept: "env-helper",
        when: When::Always,
        confidence: Confidence::High,
        severity: None,
        javascript: false,
    },
    Bound {
        rule: "async-all-the-way",
        concept: "task-wait",
        when: When::Always,
        confidence: Confidence::Low,
        severity: Some(Severity::Medium),
        javascript: false,
    },
    Bound {
        rule: "no-weak-cipher",
        concept: "weak-cipher",
        when: When::Always,
        confidence: Confidence::Medium,
        severity: None,
        javascript: false,
    },
    Bound {
        rule: "no-open-redirect",
        concept: "redirect",
        when: When::ReceivesInput,
        confidence: Confidence::High,
        severity: None,
        javascript: true,
    },
    Bound {
        rule: "no-template-injection",
        concept: "template-sink",
        when: When::ReceivesInput,
        confidence: Confidence::High,
        severity: None,
        javascript: false,
    },
    Bound {
        rule: "no-unsafe-reflection",
        concept: "reflection-sink",
        when: When::ReceivesInput,
        confidence: Confidence::High,
        severity: None,
        javascript: false,
    },
    Bound {
        rule: "no-regex-from-input",
        concept: "regex-sink",
        when: When::ReceivesInput,
        confidence: Confidence::High,
        severity: None,
        javascript: true,
    },
    Bound {
        rule: "no-unsafe-xml-parser",
        concept: "xml-parser",
        when: When::Always,
        confidence: Confidence::Low,
        severity: Some(Severity::Medium),
        javascript: false,
    },
    Bound {
        rule: "no-process-exit",
        concept: "process-exit",
        when: When::Always,
        confidence: Confidence::High,
        severity: None,
        javascript: false,
    },
    Bound {
        rule: "no-type-punning",
        concept: "type-punning",
        when: When::Always,
        confidence: Confidence::High,
        severity: None,
        javascript: false,
    },
    Bound {
        rule: "no-http-without-timeout",
        concept: "http-no-timeout",
        when: When::WithoutKeyword("timeout"),
        confidence: Confidence::High,
        severity: None,
        javascript: false,
    },
    Bound {
        rule: "no-format-string",
        concept: "format-sink",
        when: When::ReceivesInput,
        confidence: Confidence::High,
        severity: None,
        javascript: false,
    },
    Bound {
        rule: "no-extract",
        concept: "variable-extraction",
        when: When::Always,
        confidence: Confidence::High,
        severity: None,
        javascript: false,
    },
    Bound {
        rule: "no-removed-mysql-api",
        concept: "legacy-db-api",
        when: When::Always,
        confidence: Confidence::High,
        severity: None,
        javascript: false,
    },
    Bound {
        rule: "no-global-scope",
        concept: "coroutine-leak",
        when: When::Always,
        confidence: Confidence::High,
        severity: None,
        javascript: false,
    },
    Bound {
        rule: "no-run-blocking",
        concept: "run-blocking",
        when: When::Always,
        confidence: Confidence::High,
        severity: None,
        javascript: false,
    },
    Bound {
        rule: "no-gc-collect",
        concept: "gc-collect",
        when: When::Always,
        confidence: Confidence::High,
        severity: None,
        javascript: false,
    },
    Bound {
        rule: "no-dispatch-main-sync",
        concept: "main-sync",
        when: When::Always,
        confidence: Confidence::High,
        severity: None,
        javascript: false,
    },
    Bound {
        rule: "no-unfinished-code",
        concept: "unfinished-code",
        when: When::Always,
        confidence: Confidence::High,
        severity: None,
        javascript: false,
    },
    Bound {
        rule: "no-env-mutation",
        concept: "env-mutation",
        when: When::Always,
        confidence: Confidence::High,
        severity: None,
        javascript: false,
    },
    Bound {
        rule: "no-leak-memory",
        concept: "memory-leak",
        when: When::Always,
        confidence: Confidence::High,
        severity: None,
        javascript: false,
    },
    Bound {
        rule: "spring-csrf-disabled",
        concept: "csrf-disable",
        when: When::Always,
        confidence: Confidence::High,
        severity: None,
        javascript: false,
    },
    Bound {
        rule: "spring-permit-all-requests",
        concept: "permit-all-requests",
        when: When::Always,
        confidence: Confidence::High,
        severity: None,
        javascript: false,
    },
    Bound {
        rule: "aspnet-developer-exception-page",
        concept: "developer-errors",
        when: When::Always,
        confidence: Confidence::High,
        severity: None,
        javascript: false,
    },
    Bound {
        rule: "no-open-cors",
        concept: "open-cors-call",
        when: When::Always,
        confidence: Confidence::High,
        severity: None,
        javascript: false,
    },
];

/// The rules this group answers for.
pub const RULES: &[&str] = &[
    "no-eval",
    "no-panic",
    "no-shell-interpolation",
    "n-plus-one-queries",
    "a03-injection",
    "no-unsafe-deserialization",
    "no-weak-hash",
    "no-print-stack-trace",
    "no-fatal-error",
    "no-raw-markup",
    "no-permit-all",
    "no-atom-from-input",
    "no-unbounded-c-string",
    "no-debug-output",
    "no-weak-random",
    "a10-ssrf",
    "no-path-traversal",
    "no-env-outside-config",
    "async-all-the-way",
    "no-swallowed-error",
    "no-weak-cipher",
    "no-open-redirect",
    "no-template-injection",
    "no-unsafe-reflection",
    "no-regex-from-input",
    "no-unsafe-xml-parser",
    "no-process-exit",
    "no-type-punning",
    "no-http-without-timeout",
    "no-format-string",
    "no-extract",
    "no-removed-mysql-api",
    "no-global-scope",
    "no-run-blocking",
    "no-gc-collect",
    "no-dispatch-main-sync",
    "no-unfinished-code",
    "no-env-mutation",
    "no-leak-memory",
    "spring-csrf-disabled",
    "spring-permit-all-requests",
    "aspnet-developer-exception-page",
    "no-open-cors",
];

/// The first argument, when it is built from interpolation or concatenation (only spaces between
/// it and the `(`).
fn built_first_arg(call: &Call, source: &str) -> Option<Span> {
    let args = call.args?;
    let rest = source.get(args.start as usize + 1..)?;
    let first = args.start as usize + 1 + (rest.len() - rest.trim_start().len());
    call.dynamic_args
        .iter()
        .find(|d| d.start as usize == first)
        .copied()
}

/// Languages among `languages` the rule cannot read because the concept tables bind none of its
/// concepts there. JavaScript and TypeScript are left out: they have detectors of their own.
#[must_use]
pub fn unbound_languages(rule: &str, languages: &[Language]) -> Vec<&'static str> {
    let table = concepts();
    if !BOUND.iter().any(|b| b.rule == rule) {
        return Vec::new();
    }
    let mut unbound: Vec<&'static str> = languages
        .iter()
        .filter(|l| l.backend() == Backend::TreeSitter)
        .filter(|l| {
            BOUND
                .iter()
                .filter(|b| b.rule == rule)
                .all(|b| table.status(b.concept, **l) == Status::NotImplemented)
        })
        .map(|l| l.id())
        .collect();
    unbound.sort_unstable();
    unbound.dedup();
    unbound
}

/// Languages whose tables bind a concept of the rule, with the javascript flag honored.
#[must_use]
pub fn languages_of(rule: &str) -> Vec<Language> {
    let table = concepts();
    Language::ALL
        .into_iter()
        .filter(|l| {
            BOUND.iter().filter(|b| b.rule == rule).any(|b| {
                table.status(b.concept, *l) == Status::Bound
                    && (b.javascript || !matches!(l.backend(), Backend::Oxc | Backend::Sfc))
            })
        })
        .collect()
}

pub fn register(registry: &mut Registry) {
    registry.register_group(RULES, detect);
}

/// Whether the call is reported, and how sure the report is.
fn judge(
    bound: &Bound,
    call: &Call,
    model: &Model,
    flow: &Flow<'_>,
    input: &DetectorInput<'_>,
    language: Language,
) -> Option<(Confidence, Option<Severity>)> {
    let plain = Some((bound.confidence, bound.severity));
    match bound.when {
        When::Always => plain,
        When::CommandIsBuilt => {
            let built = built_first_arg(call, input.content)?;
            flow.exposes(built).then_some(()).and(plain)
        }
        When::ReadsIteration => model.reads_iteration(call).then_some(()).and(plain),
        When::WithoutKeyword(keyword) => {
            let named = call
                .args
                .and_then(|a| input.content.get(a.start as usize..a.end as usize))
                .is_some_and(|text| text.contains(keyword));
            (!named).then_some(()).and(plain)
        }
        When::WithoutOptions => (call.arg_list.len() <= 1).then_some(()).and(plain),
        When::ReceivesInput => {
            let index = concepts().argument(bound.concept, language, &call.callee)?;
            match flow.reaches(*call.arg_list.get(index)?)? {
                Certainty::Certain => plain,
                Certainty::Uncertain => Some((Confidence::Low, Some(Severity::Medium))),
            }
        }
    }
}

/// The rules over the model. A rule with `javascript` set also reports JavaScript and TypeScript
/// files; for the others those languages have a detector of their own that reads more than the
/// model does (the typed or request-shaped parts of the rule).
#[must_use]
pub fn detect(input: &DetectorInput<'_>) -> Vec<(&'static str, Hit)> {
    let Some(syntax) = input.syntax else {
        return Vec::new();
    };
    let on_javascript = matches!(syntax.language.backend(), Backend::Oxc | Backend::Sfc);
    let Some(model) = model_of(syntax, input.content) else {
        return Vec::new();
    };
    let language = syntax.language;
    let table = concepts();
    let flow = Flow::new(&model, table, language);
    let lines = Lines::new(input.content);
    let test_file = is_test_path(input.file);
    let file_name = Path::new(input.file)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default();
    let mut hits = Vec::new();
    for bound in BOUND.iter().filter(|b| b.javascript || !on_javascript) {
        let Some(meaning) = table.meaning(bound.concept, language) else {
            continue;
        };
        if meaning.skip_files.iter().any(|f| f == file_name)
            || table.skips_dir(bound.concept, input.file)
        {
            continue;
        }
        let skip_tests = table.skips_tests(bound.concept);
        for call in &model.calls {
            if !table.is(bound.concept, language, &call.callee)
                || (skip_tests && (test_file || call.in_test))
            {
                continue;
            }
            let Some((confidence, severity)) = judge(bound, call, &model, &flow, input, language)
            else {
                continue;
            };
            let start = call.callee_span.start as usize;
            let end = call.args.map_or(call.callee_span.end, |a| a.start + 1) as usize;
            if let Some(mut hit) = make_hit(input, &lines, start, end, confidence, None) {
                hit.severity = severity;
                hits.push((bound.rule, hit, start));
            }
        }
    }
    for handler in model.empty_handlers.iter().filter(|_| !test_file) {
        let (start, end) = (handler.start as usize, handler.end as usize);
        if let Some(hit) = make_hit(input, &lines, start, end, Confidence::High, None) {
            hits.push(("no-swallowed-error", hit, start));
        }
    }
    one_per_line(hits)
}
