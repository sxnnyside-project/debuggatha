//! Inline suppression: a comment that says, in the code itself, that a finding is accepted and
//! why. It reads the way a person would write it:
//!
//! ```text
//! risky(); // debuggatha-ignore no-eval -- sandboxed plugin loader
//! // debuggatha-ignore-next-line eqeqeq, no-var -- legacy vendored shim
//! // debuggatha-ignore-file no-explicit-any -- generated API types
//! ```
//!
//! With no rule ids the directive covers every rule on its target. The
//! patterns run on `regress` so `\s`, `\w` and `.` mean what they mean in JavaScript.

use std::sync::LazyLock;

use debuggatha_core::js::js_trim;
use regress::Regex;

use super::{Span, SyntaxFacts};

/// What a directive covers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirectiveScope {
    /// The line it is on.
    Line,
    /// The line after it.
    NextLine,
    /// The whole file.
    File,
}

/// One line of a comment, without its delimiters. A block comment contributes one entry per line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommentLine {
    /// 1-based line in the reviewed text.
    pub line: u32,
    /// The comment's text.
    pub text: String,
}

/// One `debuggatha-ignore` comment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Directive {
    /// What it covers.
    pub scope: DirectiveScope,
    /// The line it is on.
    pub line: u32,
    /// `None` covers every rule.
    pub rule_ids: Option<Vec<String>>,
    /// Why the finding is accepted.
    pub reason: Option<String>,
}

/// What an inline directive hid, reported next to the findings so nothing is silently dropped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InlineSuppression {
    /// The file.
    pub file: String,
    /// The line, when the directive names one.
    pub line: Option<u32>,
    /// The rule.
    pub rule_id: String,
    /// The reason given.
    pub reason: Option<String>,
}

#[allow(clippy::expect_used)] // constant patterns, proven by the tests
static DIRECTIVE: LazyLock<Regex> = LazyLock::new(|| {
    // debuggatha-ignore-next-line no-unwrap-expect -- constant pattern, proven by the tests
    Regex::new(r"debuggatha-ignore(-next-line|-file)?\b(.*)$").expect("a valid pattern")
});
#[allow(clippy::expect_used)] // constant pattern, proven by the tests
static REASON_SEPARATOR: LazyLock<Regex> =
    // debuggatha-ignore-next-line no-unwrap-expect -- constant pattern, proven by the tests
    LazyLock::new(|| Regex::new(r"\s--\s|\s—\s").expect("a valid pattern"));
#[allow(clippy::expect_used)] // constant pattern, proven by the tests
static RULE_SEPARATOR: LazyLock<Regex> =
    // debuggatha-ignore-next-line no-unwrap-expect -- constant pattern, proven by the tests
    LazyLock::new(|| Regex::new(r"[\s,:]+").expect("a valid pattern"));
#[allow(clippy::expect_used)] // constant pattern, proven by the tests
static RULE_ID: LazyLock<Regex> =
    // debuggatha-ignore-next-line no-unwrap-expect -- constant pattern, proven by the tests
    LazyLock::new(|| Regex::new(r"^[\w*-]+$").expect("a valid pattern"));
#[allow(clippy::expect_used)] // constant pattern, proven by the tests
static BLOCK_CLOSE: LazyLock<Regex> =
    // debuggatha-ignore-next-line no-unwrap-expect -- constant pattern, proven by the tests
    LazyLock::new(|| Regex::new(r"\*/\s*$").expect("a valid pattern"));

/// JavaScript's `String.prototype.split(regex)` for a pattern that cannot match the empty string.
fn split<'a>(re: &Regex, text: &'a str) -> Vec<&'a str> {
    let mut parts = Vec::new();
    let mut from = 0;
    for m in re.find_iter(text) {
        parts.push(&text[from..m.start()]);
        from = m.end();
    }
    parts.push(&text[from..]);
    parts
}

/// Read the directives out of comment lines.
#[must_use]
pub fn parse_directives(comments: &[CommentLine]) -> Vec<Directive> {
    let mut directives = Vec::new();
    for comment in comments {
        let Some(m) = DIRECTIVE.find(&comment.text) else {
            continue;
        };
        let scope = match m.group(1).map(|r| &comment.text[r]) {
            Some("-next-line") => DirectiveScope::NextLine,
            Some("-file") => DirectiveScope::File,
            _ => DirectiveScope::Line,
        };
        let rest = m.group(2).map_or("", |r| &comment.text[r]);

        let parts = split(&REASON_SEPARATOR, rest);
        let rule_part = parts.first().copied().unwrap_or("");
        let mut reason = parts.get(1..).unwrap_or_default().join(" -- ");
        if let Some(close) = BLOCK_CLOSE.find(&reason) {
            reason.truncate(close.start());
        }
        let reason = js_trim(&reason);

        let ids: Vec<String> = split(&RULE_SEPARATOR, rule_part)
            .into_iter()
            .map(js_trim)
            .filter(|id| RULE_ID.find(id).is_some())
            .map(str::to_owned)
            .collect();

        directives.push(Directive {
            scope,
            line: comment.line,
            rule_ids: if ids.is_empty() || ids.iter().any(|id| id == "*") {
                None
            } else {
                Some(ids)
            },
            reason: if reason.is_empty() {
                None
            } else {
                Some(reason.to_owned())
            },
        });
    }
    directives
}

/// The directive that covers `rule_id` on `line` (1-based), if any.
#[must_use]
pub fn find_directive<'a>(
    directives: &'a [Directive],
    line: u32,
    rule_id: &str,
) -> Option<&'a Directive> {
    directives.iter().find(|d| {
        if d.rule_ids
            .as_ref()
            .is_some_and(|ids| !ids.iter().any(|id| id == rule_id))
        {
            return false;
        }
        match d.scope {
            DirectiveScope::File => true,
            DirectiveScope::Line => d.line == line,
            DirectiveScope::NextLine => d.line + 1 == line,
        }
    })
}

/// The comment that accepts one rule on the line after it, in the syntax of the file's language.
/// The reason is required text, not decoration: an accepted finding with no why is how a
/// suppression list turns into a graveyard.
#[must_use]
pub fn inline_suppression_comment(file: &str, rule_id: &str, reason: &str) -> String {
    let body = format!("debuggatha-ignore-next-line {rule_id} -- {reason}");
    let ext = file
        .rsplit_once('.')
        .map_or("", |(_, e)| e)
        .to_ascii_lowercase();
    match ext.as_str() {
        "py" | "rb" | "sh" | "yaml" | "yml" | "toml" | "env" => format!("# {body}"),
        "html" | "htm" | "vue" | "svelte" | "md" => format!("<!-- {body} -->"),
        _ => format!("// {body}"),
    }
}

/// The lines of the comments the parser found, in the shape directives are read from. A trailing
/// `\r` is dropped: the TypeScript lexer kept it, and `.` in its pattern then refused to match, so
/// a directive in a CRLF file was silently ignored (see `docs/PARITY.md`).
#[must_use]
pub fn comment_lines(source: &str, facts: &SyntaxFacts) -> Vec<CommentLine> {
    let mut out = Vec::new();
    for Span { start, end } in &facts.comments {
        let (start, end) = (*start as usize, *end as usize);
        let Some(text) = source.get(start..end) else {
            continue;
        };
        let first_line = 1 + source[..start].bytes().filter(|b| *b == b'\n').count();
        let body = if let Some(rest) = text.strip_prefix("//") {
            rest
        } else if let Some(rest) = text.strip_prefix("/*") {
            rest.strip_suffix("*/").unwrap_or(rest)
        } else if let Some(rest) = text.strip_prefix('#') {
            rest
        } else if let Some(rest) = text.strip_prefix("<!--") {
            rest.strip_suffix("-->").unwrap_or(rest)
        } else {
            text
        };
        for (offset, line) in body.split('\n').enumerate() {
            out.push(CommentLine {
                line: u32::try_from(first_line + offset).unwrap_or(u32::MAX),
                text: line.strip_suffix('\r').unwrap_or(line).to_owned(),
            });
            if text.starts_with("//") || text.starts_with('#') {
                break; // a line comment is one line
            }
        }
    }
    out
}
