//! The query runner: a tree-sitter query (`.scm`) compiled once for a language and run over a
//! parsed file. Syntactic detectors for the tree-sitter languages are written as queries next to
//! their pack (ADR-0006, decision 3); the runner is the only code that touches `QueryCursor`.
//!
//! Text predicates (`#eq?`, `#match?`, `#any-of?`) are evaluated by tree-sitter. A match that
//! touches a region the parser could not read is dropped by the caller with
//! `FileSyntax::touches_error`, never concluded about.

use std::fmt;

use streaming_iterator::StreamingIterator;
use tree_sitter::{QueryCursor, TextProvider};

use super::language::Language;
use super::treesitter::grammar;
use super::{FileSyntax, Span};

#[derive(Debug)]
pub struct Query {
    language: Language,
    inner: tree_sitter::Query,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QueryError {
    /// The language is parsed by oxc, whose detectors are code and not queries.
    NotTreeSitter(Language),
    /// The query does not compile for the grammar: where and why.
    Invalid {
        row: usize,
        column: usize,
        message: String,
    },
    /// `run` was given a file of another language than the query's.
    WrongLanguage { query: Language, file: Language },
    /// The file has no tree (it failed to parse).
    NoTree,
}

impl fmt::Display for QueryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotTreeSitter(l) => write!(f, "{} is not parsed by tree-sitter", l.id()),
            Self::Invalid {
                row,
                column,
                message,
            } => {
                write!(f, "invalid query at {}:{}: {message}", row + 1, column + 1)
            }
            Self::WrongLanguage { query, file } => {
                write!(
                    f,
                    "a {} query cannot run on a {} file",
                    query.id(),
                    file.id()
                )
            }
            Self::NoTree => f.write_str("the file has no syntax tree"),
        }
    }
}

impl std::error::Error for QueryError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Capture {
    pub name: String,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryMatch {
    /// Index of the pattern in the query source, in order.
    pub pattern: usize,
    pub captures: Vec<Capture>,
}

impl Query {
    /// # Errors
    /// The language has no tree-sitter grammar, or the query does not compile against it.
    pub fn new(language: Language, source: &str) -> Result<Self, QueryError> {
        let grammar = grammar(language).ok_or(QueryError::NotTreeSitter(language))?;
        let inner = tree_sitter::Query::new(&grammar, source).map_err(|e| QueryError::Invalid {
            row: e.row,
            column: e.column,
            message: e.message,
        })?;
        Ok(Self { language, inner })
    }

    /// Every match, in source order.
    ///
    /// # Errors
    /// The file is of another language, or has no tree.
    pub fn run(&self, file: &FileSyntax, source: &str) -> Result<Vec<QueryMatch>, QueryError> {
        if file.language != self.language {
            return Err(QueryError::WrongLanguage {
                query: self.language,
                file: file.language,
            });
        }
        let tree = file.tree.as_ref().ok_or(QueryError::NoTree)?;
        let mut cursor = QueryCursor::new();
        let names = self.inner.capture_names();
        let mut out = Vec::new();
        let mut matches = cursor.matches(&self.inner, tree.root_node(), Bytes(source.as_bytes()));
        while let Some(m) = matches.next() {
            out.push(QueryMatch {
                pattern: m.pattern_index,
                captures: m
                    .captures()
                    .iter()
                    .map(|c| Capture {
                        name: names[c.index as usize].to_owned(),
                        span: Span::new(
                            u32::try_from(c.node.start_byte()).unwrap_or(u32::MAX),
                            u32::try_from(c.node.end_byte()).unwrap_or(u32::MAX),
                        ),
                    })
                    .collect(),
            });
        }
        Ok(out)
    }
}

struct Bytes<'a>(&'a [u8]);

impl<'a> TextProvider<&'a [u8]> for Bytes<'a> {
    type I = std::iter::Once<&'a [u8]>;
    fn text(&mut self, node: tree_sitter::Node<'_>) -> Self::I {
        std::iter::once(self.0.get(node.byte_range()).unwrap_or_default())
    }
}
