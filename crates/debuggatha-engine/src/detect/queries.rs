//! Rules of single languages, written as queries (`data/queries/*.scm`) next to the code that
//! reads their captures: Rust `no-unwrap-expect` and `no-unsafe-blocks`, Dart `avoid-print`.
//!
//! A Rust macro body is only tokens to the parser, so the macro patterns read tokens. `.expect(a, b)`
//! with other than one argument is the program's own method, and a Dart `example/`, `tool/` or
//! `bin/` script prints as its output.

use std::path::Path;
use std::sync::OnceLock;

use super::calls::one_per_line;
use super::hit::Hit;
use super::input::DetectorInput;
use super::js::{Lines, make_hit};
use super::runner::Registry;
use crate::scan_scope::is_test_path;
use crate::syntax::{FileSyntax, Language, Query, QueryMatch, Span};
use debuggatha_core::review_engine::{Confidence, EditSafety};

/// The rules this group answers for.
pub const RULES: &[&str] = &["no-unwrap-expect", "no-unsafe-blocks", "avoid-print"];

/// Registers the group.
pub fn register(registry: &mut Registry) {
    registry.register_group(RULES, detect);
}

const SOURCES: [(Language, &str); 2] = [
    (Language::Rust, include_str!("../../data/queries/rust.scm")),
    (Language::Dart, include_str!("../../data/queries/dart.scm")),
];

/// Compiled on first use: a repository holds a few languages, and every query costs milliseconds.
fn compiled(index: usize) -> &'static Result<Query, String> {
    static QUERIES: [OnceLock<Result<Query, String>>; SOURCES.len()] =
        [const { OnceLock::new() }; SOURCES.len()];
    QUERIES[index].get_or_init(|| {
        let (language, source) = SOURCES[index];
        Query::new(language, source).map_err(|e| e.to_string())
    })
}

/// Why a query does not compile, if any (a test asserts there is none).
#[must_use]
pub fn compile_errors() -> Vec<String> {
    SOURCES
        .iter()
        .enumerate()
        .filter_map(|(i, (l, _))| {
            compiled(i)
                .as_ref()
                .err()
                .map(|e| format!("{}: {e}", l.id()))
        })
        .collect()
}

fn query_for(language: Language) -> Option<&'static Query> {
    SOURCES
        .iter()
        .position(|(l, _)| *l == language)
        .and_then(|i| compiled(i).as_ref().ok())
}

fn capture(m: &QueryMatch, name: &str) -> Option<Span> {
    m.captures.iter().find(|c| c.name == name).map(|c| c.span)
}

fn range(s: Span) -> (usize, usize) {
    (s.start as usize, s.end as usize)
}

struct Reader<'a> {
    input: &'a DetectorInput<'a>,
    lines: Lines<'a>,
    hits: Vec<(&'static str, Hit, usize)>,
    /// Items under `#[test]` / `#[cfg(test)]`, and whether the file is a test file by its path.
    test_items: Vec<(usize, usize)>,
    test_file: bool,
    /// A Dart script or example, where `print` is the program's output.
    dart_script: bool,
}

impl Reader<'_> {
    fn text(&self, s: Span) -> &str {
        let (a, b) = range(s);
        self.input.content.get(a..b).unwrap_or("")
    }

    fn in_test(&self, at: usize) -> bool {
        self.test_file || self.test_items.iter().any(|&(a, b)| a <= at && at < b)
    }

    fn push(
        &mut self,
        rule: &'static str,
        (start, end): (usize, usize),
        confidence: Confidence,
        edit: Option<(usize, usize, &str, EditSafety)>,
    ) {
        if let Some(hit) = make_hit(self.input, &self.lines, start, end, confidence, edit) {
            self.hits.push((rule, hit, start));
        }
    }

    /// The `.` right before a method name.
    fn dot_before(&self, method: Span) -> usize {
        let before = self.input.content[..method.start as usize].trim_end();
        if before.ends_with('.') {
            before.len() - 1
        } else {
            method.start as usize
        }
    }

    fn rust(&mut self, matches: &[QueryMatch]) {
        for m in matches {
            if matches!(m.pattern, 5 | 6)
                && let Some(item) = capture(m, "item")
            {
                self.test_items.push(range(item));
            }
        }
        for m in matches {
            match m.pattern {
                0 | 1 | 7 => self.unwrap_or_expect(m),
                2 => {
                    if let Some(block) = capture(m, "block") {
                        // `unsafe`, spaces, then the brace the old pattern ended on.
                        let text = self.text(block);
                        if let Some(brace) = text.find('{') {
                            let start = block.start as usize;
                            self.push(
                                "no-unsafe-blocks",
                                (start, start + brace + 1),
                                Confidence::High,
                                None,
                            );
                        }
                    }
                }
                3 => {
                    if let Some(kw) = capture(m, "kw") {
                        // Only `unsafe fn`: `unsafe extern "C" fn` is not the old shape.
                        let rest = &self.input.content[kw.end as usize..];
                        let word = rest.trim_start();
                        if rest.len() > word.len() && word.starts_with("fn") {
                            let end = kw.end as usize + (rest.len() - word.len()) + 2;
                            self.push(
                                "no-unsafe-blocks",
                                (kw.start as usize, end),
                                Confidence::High,
                                None,
                            );
                        }
                    }
                }
                4 => {
                    if let (Some(kw), Some(body)) = (capture(m, "kw"), capture(m, "body"))
                        && self.text(body).starts_with('{')
                    {
                        self.push(
                            "no-unsafe-blocks",
                            (kw.start as usize, body.start as usize + 1),
                            Confidence::High,
                            None,
                        );
                    }
                }
                _ => {}
            }
        }
    }

    fn unwrap_or_expect(&mut self, m: &QueryMatch) {
        let (Some(method), Some(args)) = (capture(m, "method"), capture(m, "args")) else {
            return;
        };
        let inner = self.text(args);
        if !inner.starts_with('(') {
            return;
        }
        // A plain `.expect(..)` call is judged by pattern 7, which counts its arguments.
        if m.pattern == 0 && self.text(method) == "expect" {
            return;
        }
        let start = self.dot_before(method);
        if self.in_test(start) {
            return;
        }
        let end = if self.text(method) == "unwrap" {
            // `.unwrap()`: nothing but space between the parentheses.
            if !inner[1..inner.len() - 1].trim().is_empty() {
                return;
            }
            args.end as usize
        } else {
            args.start as usize + 1
        };
        self.push("no-unwrap-expect", (start, end), Confidence::High, None);
    }

    /// A plain call to `name(`.
    fn call(
        &mut self,
        matches: &[QueryMatch],
        rule: &'static str,
        edit: Option<(&str, EditSafety)>,
    ) {
        for m in matches {
            let (Some(name), Some(args)) = (capture(m, "name"), capture(m, "args")) else {
                continue;
            };
            let start = name.start as usize;
            if rule == "avoid-print" && self.dart_script {
                continue;
            }
            let edit =
                edit.map(|(replacement, safety)| (start, name.end as usize, replacement, safety));
            self.push(
                rule,
                (start, args.start as usize + 1),
                Confidence::High,
                edit,
            );
        }
    }
}

/// Runs the group over one file.
#[must_use]
pub fn detect(input: &DetectorInput<'_>) -> Vec<(&'static str, Hit)> {
    let Some(syntax) = input.syntax else {
        return Vec::new();
    };
    let language = syntax.language;
    let Some(query) = query_for(language) else {
        return Vec::new();
    };
    let Ok(matches) = query.run(syntax as &FileSyntax, input.content) else {
        return Vec::new();
    };
    let mut reader = Reader {
        input,
        lines: Lines::new(input.content),
        hits: Vec::new(),
        test_items: Vec::new(),
        test_file: is_test_path(input.file),
        dart_script: Path::new(input.file).components().any(|c| {
            ["example", "examples", "tool", "bin"]
                .iter()
                .any(|d| c.as_os_str() == *d)
        }),
    };
    match language {
        Language::Rust => reader.rust(&matches),
        Language::Dart => reader.call(
            &matches,
            "avoid-print",
            Some(("debugPrint", EditSafety::Safe)),
        ),
        _ => {}
    }
    one_per_line(reader.hits)
}

#[cfg(test)]
mod tests {
    #[test]
    fn every_query_compiles() {
        assert_eq!(super::compile_errors(), Vec::<String>::new());
    }
}
