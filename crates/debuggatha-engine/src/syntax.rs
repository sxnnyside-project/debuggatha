//! The syntax layer: parse a file once, learn what is code and what is not, and say honestly how
//! well the parse went.
//!
//! A file parsed **with errors** keeps its facts (oxc recovers) but also its error spans, so a
//! detector can leave the broken region alone: nothing is concluded about it. A file that could
//! not be parsed at all has no facts.

mod language;
mod mask;
pub(crate) mod oxc;
mod query;
mod query_set;
mod sfc;
mod suppress;
pub(crate) mod treesitter;

use std::path::Path;

pub use language::{Backend, Language};
pub use mask::mask_source;
pub use query::{Capture, Query, QueryError, QueryMatch};
pub use query_set::QuerySet;
pub use sfc::{ScriptRegion, script_regions};
pub use suppress::{
    CommentLine, Directive, DirectiveScope, InlineSuppression, comment_lines, find_directive,
    inline_suppression_comment, parse_directives,
};

/// A byte range in the source (UTF-8 offsets, `end` exclusive).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    /// First byte.
    pub start: u32,
    /// Byte after the last one.
    pub end: u32,
}

impl Span {
    /// A span from `start` to `end`.
    #[must_use]
    pub const fn new(start: u32, end: u32) -> Self {
        Self { start, end }
    }

    /// Whether the two spans share a byte.
    #[must_use]
    pub const fn overlaps(self, other: Self) -> bool {
        self.start < other.end && other.start < self.end
    }
}

/// How well a file was parsed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseStatus {
    /// No syntax errors.
    Parsed,
    /// Syntax errors in some regions; nothing is concluded about them.
    ParsedWithErrors,
    /// A fatal error: no tree, no facts.
    Failed,
}

/// What is not code. Template literals contribute only their text parts, never `${...}`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SyntaxFacts {
    /// The comments.
    pub comments: Vec<Span>,
    /// Whole string literals, delimiters included.
    pub strings: Vec<Span>,
    /// Regular expression literals.
    pub regexes: Vec<Span>,
    /// What `mask_source` hides: comments, string contents and regex patterns.
    pub hidden: Vec<Span>,
}

/// What the syntax layer knows of one file.
#[derive(Debug, Clone)]
pub struct FileSyntax {
    /// The language.
    pub language: Language,
    /// How well it parsed.
    pub status: ParseStatus,
    /// How many syntax errors.
    pub errors: usize,
    /// Where they are.
    pub error_spans: Vec<Span>,
    /// What is code and what is not, when the parse produced a tree.
    pub facts: Option<SyntaxFacts>,
    /// The tree-sitter tree, kept for queries. The oxc backend keeps none: its detectors are Rust
    /// code over its AST and run during the parse (next slices).
    tree: Option<tree_sitter::Tree>,
}

impl FileSyntax {
    /// Is the range inside a region the parser could not make sense of?
    #[must_use]
    pub fn touches_error(&self, span: Span) -> bool {
        self.error_spans.iter().any(|e| e.overlaps(span))
    }
}

/// Parse `source`, or `None` when the engine has no parser for the file's language (which a
/// review must report as not covered).
#[must_use]
pub fn parse(path: &Path, source: &str) -> Option<FileSyntax> {
    let language = Language::from_path(path)?;
    Some(match language.backend() {
        Backend::Sfc => sfc::parse(source, language),
        Backend::TreeSitter => {
            let (mut syntax, tree) = treesitter::parse(source, language);
            syntax.tree = tree;
            syntax
        }
        Backend::Oxc => {
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or_default();
            let declaration = [".d.ts", ".d.mts", ".d.cts"]
                .iter()
                .any(|e| name.ends_with(e));
            oxc::parse_js(source, language, declaration, false)
        }
    })
}

#[cfg(test)]
// A failed expect is a failed test, which is the intent here.
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    fn p(file: &str, src: &str) -> FileSyntax {
        parse(Path::new(file), src).expect("supported")
    }

    #[test]
    fn clean_file_is_parsed_with_facts() {
        let s = p("a.ts", "const a = 'x'; // c\n");
        assert_eq!(s.status, ParseStatus::Parsed);
        let f = s.facts.expect("facts");
        assert_eq!((f.comments.len(), f.strings.len()), (1, 1));
    }

    #[test]
    fn recoverable_errors_keep_facts_and_report_the_broken_region() {
        // A `const` without initializer is reported but the parser recovers.
        let src = "const a;\nconst ok = process.env.X;\n";
        let s = p("a.ts", src);
        assert_eq!(s.status, ParseStatus::ParsedWithErrors);
        assert!(s.errors >= 1 && s.facts.is_some());
        let broken = Span::new(0, 8);
        assert!(s.touches_error(broken));
        let ok_at = u32::try_from(src.find("process").expect("present")).expect("small");
        assert!(
            !s.touches_error(Span::new(ok_at, ok_at + 11)),
            "the intact line is not in an error region"
        );
    }

    #[test]
    fn fatal_error_has_no_facts() {
        let s = p("a.ts", "const a = `unterminated");
        assert_ne!(s.status, ParseStatus::Parsed);
        if s.status == ParseStatus::Failed {
            assert!(s.facts.is_none());
        }
    }

    #[test]
    fn declaration_files_allow_ambient_forms() {
        let src = "declare const x: number;\nexport declare function f(): void;\n";
        assert_eq!(p("a.d.ts", src).status, ParseStatus::Parsed);
    }

    #[test]
    fn unsupported_language_is_none_not_a_guess() {
        assert!(parse(Path::new("main.zig"), "pub fn main() void {}").is_none());
    }
}
