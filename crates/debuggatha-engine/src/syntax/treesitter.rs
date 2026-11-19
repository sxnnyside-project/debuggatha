//! The tree-sitter backend for every language outside the JavaScript/TypeScript family. Grammar
//! crates are pinned in `Cargo.lock`; each has a load test (ADR-0006, decision 4).
//!
//! What counts as a comment, a string and a string's text is a table per grammar, because
//! the node names differ. A string literal is hidden by its text parts where the grammar has them
//! (so interpolations stay code), and by its delimiters otherwise.

use tree_sitter::{Node, Parser, Tree};

use super::language::Language;
use super::{FileSyntax, ParseStatus, Span, SyntaxFacts};

pub(crate) fn grammar(language: Language) -> Option<tree_sitter::Language> {
    Some(match language {
        Language::Rust => tree_sitter_rust::LANGUAGE.into(),
        Language::Python => tree_sitter_python::LANGUAGE.into(),
        Language::Go => tree_sitter_go::LANGUAGE.into(),
        Language::Php => tree_sitter_php::LANGUAGE_PHP.into(),
        Language::Java => tree_sitter_java::LANGUAGE.into(),
        Language::CSharp => tree_sitter_c_sharp::LANGUAGE.into(),
        Language::Ruby => tree_sitter_ruby::LANGUAGE.into(),
        Language::Kotlin => tree_sitter_kotlin_ng::LANGUAGE.into(),
        Language::Swift => tree_sitter_swift::LANGUAGE.into(),
        Language::Dart => tree_sitter_dart::LANGUAGE.into(),
        Language::Elixir => tree_sitter_elixir::LANGUAGE.into(),
        Language::Cpp => tree_sitter_cpp::LANGUAGE.into(),
        Language::GdScript => tree_sitter_gdscript::LANGUAGE.into(),
        Language::Html => tree_sitter_html::LANGUAGE.into(),
        Language::JavaScript
        | Language::Jsx
        | Language::TypeScript
        | Language::Tsx
        | Language::Vue
        | Language::Svelte
        | Language::Astro => return None,
    })
}

/// Node kinds that matter for one grammar.
struct Kinds {
    /// A comment: hidden whole, not descended into.
    comments: &'static [&'static str],
    /// A whole string literal (recorded in `strings`).
    literals: &'static [&'static str],
    /// The text of a string: hidden verbatim.
    text: &'static [&'static str],
    /// A literal the grammar gives no text node for: hidden between its delimiters, except for
    /// the children in `code`.
    stripped: &'static [&'static str],
    /// Children of a stripped literal that are code (interpolations).
    code: &'static [&'static str],
}

const ESC: &str = "escape_sequence";

// A table of grammar node names, one arm per language: splitting it would only scatter the data.
#[allow(clippy::too_many_lines)]
fn kinds(language: Language) -> Kinds {
    match language {
        Language::Rust => Kinds {
            comments: &["line_comment", "block_comment"],
            literals: &["string_literal", "raw_string_literal"],
            text: &["string_content", ESC],
            stripped: &["char_literal"],
            code: &[],
        },
        Language::Python => Kinds {
            comments: &["comment"],
            literals: &["string"],
            text: &["string_content", ESC],
            stripped: &[],
            code: &[],
        },
        Language::Go => Kinds {
            comments: &["comment"],
            literals: &["interpreted_string_literal", "raw_string_literal"],
            text: &[
                "interpreted_string_literal_content",
                "raw_string_literal_content",
                ESC,
            ],
            stripped: &[],
            code: &[],
        },
        Language::Php => Kinds {
            comments: &["comment"],
            literals: &["string", "encapsed_string", "nowdoc_string", "heredoc"],
            text: &["string_content", ESC],
            stripped: &[],
            code: &[],
        },
        Language::Java => Kinds {
            comments: &["line_comment", "block_comment"],
            literals: &["string_literal"],
            text: &["string_fragment", "multiline_string_fragment", ESC],
            stripped: &["character_literal"],
            code: &[],
        },
        Language::CSharp => Kinds {
            comments: &["comment"],
            literals: &[
                "string_literal",
                "verbatim_string_literal",
                "raw_string_literal",
                "interpolated_string_expression",
            ],
            text: &[
                "string_content",
                "string_literal_content",
                "raw_string_content",
                ESC,
            ],
            stripped: &["character_literal"],
            code: &[],
        },
        Language::Ruby => Kinds {
            comments: &["comment"],
            literals: &["string", "bare_string"],
            text: &["string_content", "heredoc_content", ESC],
            stripped: &[],
            code: &[],
        },
        Language::Kotlin => Kinds {
            comments: &["line_comment", "block_comment"],
            literals: &["string_literal", "multiline_string_literal"],
            text: &["string_content", ESC],
            stripped: &[],
            code: &[],
        },
        Language::Swift => Kinds {
            comments: &["comment", "multiline_comment"],
            literals: &[],
            text: &[],
            stripped: &[
                "line_string_literal",
                "multi_line_string_literal",
                "raw_string_literal",
            ],
            code: &["interpolated_expression", "raw_str_interpolation"],
        },
        Language::Dart => Kinds {
            comments: &[
                "comment",
                "documentation_comment",
                "block_comment",
                "documentation_block_comment",
            ],
            literals: &["string_literal"],
            text: &[
                "template_chars_double",
                "template_chars_double_single",
                "template_chars_raw_slash",
                "template_chars_single",
                "template_chars_single_single",
                ESC,
            ],
            stripped: &[
                "raw_string_literal_double_quotes",
                "raw_string_literal_double_quotes_multiple",
                "raw_string_literal_single_quotes",
                "raw_string_literal_single_quotes_multiple",
            ],
            code: &[],
        },
        Language::Elixir => Kinds {
            comments: &["comment"],
            literals: &["string", "charlist", "sigil"],
            text: &["quoted_content", ESC],
            stripped: &[],
            code: &[],
        },
        Language::Cpp => Kinds {
            comments: &["comment"],
            literals: &["string_literal", "raw_string_literal"],
            text: &["string_content", "raw_string_content", ESC],
            stripped: &["char_literal"],
            code: &[],
        },
        Language::GdScript => Kinds {
            comments: &["comment"],
            literals: &[],
            text: &[ESC],
            stripped: &["string", "string_name"],
            code: &[],
        },
        Language::Html => Kinds {
            comments: &["comment"],
            literals: &[],
            text: &[],
            stripped: &["quoted_attribute_value"],
            code: &[],
        },
        Language::JavaScript
        | Language::Jsx
        | Language::TypeScript
        | Language::Tsx
        | Language::Vue
        | Language::Svelte
        | Language::Astro => Kinds {
            comments: &[],
            literals: &[],
            text: &[],
            stripped: &[],
            code: &[],
        },
    }
}

fn span(node: Node<'_>) -> Span {
    Span::new(
        u32::try_from(node.start_byte()).unwrap_or(u32::MAX),
        u32::try_from(node.end_byte()).unwrap_or(u32::MAX),
    )
}

/// The range between the delimiters of a literal: a leading run of prefix letters, `#` and quotes,
/// and the matching trailing run. Good enough to hide the text of `"x"`, `'x'`, `r#"x"#`,
/// `"""x"""` and `b'x'`; interpolations are carved out by the caller.
fn inner(source: &str, node: Node<'_>) -> Option<(usize, usize)> {
    let text = source.get(node.start_byte()..node.end_byte())?;
    let is_delim = |c: char| matches!(c, '"' | '\'' | '`' | '#');
    let lead = text.find(|c: char| is_delim(c) && c != '#')?;
    if text[..lead].chars().any(|c| !c.is_ascii_alphabetic()) {
        return None;
    }
    let open = text[lead..]
        .find(|c: char| !is_delim(c))
        .map_or(text.len(), |i| lead + i);
    let close = text.rfind(|c: char| !is_delim(c)).map_or(open, |i| {
        i + text[i..].chars().next().map_or(1, char::len_utf8)
    });
    (open <= close).then_some((node.start_byte() + open, node.start_byte() + close))
}

struct Walk<'s> {
    source: &'s str,
    k: Kinds,
    facts: SyntaxFacts,
    error_spans: Vec<Span>,
}

impl Walk<'_> {
    fn visit(&mut self, node: Node<'_>) {
        let kind = node.kind();
        if node.is_error() || node.is_missing() {
            let s = span(node);
            self.error_spans
                .push(Span::new(s.start, s.end.max(s.start + 1)));
        }
        if self.k.comments.contains(&kind) {
            self.facts.comments.push(span(node));
            self.facts.hidden.push(span(node));
            return;
        }
        if self.k.literals.contains(&kind) {
            self.facts.strings.push(span(node));
        }
        if self.k.text.contains(&kind) {
            self.facts.hidden.push(span(node));
        }
        if self.k.stripped.contains(&kind) {
            self.facts.strings.push(span(node));
            self.hide_stripped(node);
        }
        // Only a subtree with an error needs a second look for error nodes; the rest is walked
        // for its strings and comments.
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.visit(child);
        }
    }

    fn hide_stripped(&mut self, node: Node<'_>) {
        let Some((mut from, to)) = inner(self.source, node) else {
            return;
        };
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if self.k.code.contains(&child.kind()) {
                if child.start_byte() > from {
                    self.facts.hidden.push(Span::new(
                        u32::try_from(from).unwrap_or(u32::MAX),
                        u32::try_from(child.start_byte()).unwrap_or(u32::MAX),
                    ));
                }
                from = from.max(child.end_byte());
            }
        }
        if to > from {
            self.facts.hidden.push(Span::new(
                u32::try_from(from).unwrap_or(u32::MAX),
                u32::try_from(to).unwrap_or(u32::MAX),
            ));
        }
    }
}

pub(super) fn parse(source: &str, language: Language) -> (FileSyntax, Option<Tree>) {
    let failed = |errors| FileSyntax {
        language,
        status: ParseStatus::Failed,
        errors,
        error_spans: Vec::new(),
        facts: None,
        tree: None,
    };
    let Some(grammar) = grammar(language) else {
        return (failed(1), None);
    };
    let mut parser = Parser::new();
    if parser.set_language(&grammar).is_err() {
        return (failed(1), None);
    }
    let Some(tree) = parser.parse(source, None) else {
        return (failed(1), None);
    };

    let mut walk = Walk {
        source,
        k: kinds(language),
        facts: SyntaxFacts::default(),
        error_spans: Vec::new(),
    };
    walk.visit(tree.root_node());
    let errors = walk.error_spans.len();
    let status = if tree.root_node().has_error() {
        ParseStatus::ParsedWithErrors
    } else {
        ParseStatus::Parsed
    };
    (
        FileSyntax {
            language,
            status,
            errors,
            error_spans: walk.error_spans,
            facts: Some(walk.facts),
            tree: None,
        },
        Some(tree),
    )
}
