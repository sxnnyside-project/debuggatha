//! Single-file components (Vue, Svelte, Astro): the script blocks are extracted with the HTML
//! grammar and parsed by oxc as JavaScript or TypeScript; their facts are moved back to offsets in
//! the file, so a line, a column or a suppression comment means what it means in the editor.
//! Templates and styles are not parsed (ADR-0006, decision 4): `Language::limited()` says so and
//! a review must report it as not covered.
//!
//! Astro reads its client scripts as TypeScript unless told otherwise, and its frontmatter may
//! `return`; Vue and Svelte read a script as JavaScript unless `lang` says `ts`.
//!
//! A block that cannot be parsed (a fatal error, or a language other than JavaScript and
//! TypeScript) becomes an error region of the file and nothing is concluded about it; the other
//! blocks keep their facts.

use tree_sitter::{Node, Parser};

use super::language::Language;
use super::treesitter::grammar;
use super::{FileSyntax, ParseStatus, Span, SyntaxFacts, oxc};

struct Block {
    start: usize,
    end: usize,
    language: Option<Language>,
    /// An Astro frontmatter may `return` (a redirect, a 404).
    frontmatter: bool,
}

pub(super) fn parse(source: &str, language: Language) -> FileSyntax {
    let mut facts = SyntaxFacts::default();
    let mut error_spans = Vec::new();
    let mut errors = 0;
    let mut blocks = Vec::new();

    // Astro keeps a TypeScript frontmatter between two `---` lines at the top. It is blanked out
    // before the HTML parse so a `<script>` inside one of its strings is not taken for a block.
    let mut html = source.to_owned();
    if language == Language::Astro
        && let Some((start, end)) = astro_frontmatter(source)
    {
        blocks.push(Block {
            start,
            end,
            language: Some(Language::TypeScript),
            frontmatter: true,
        });
        html = blank(source, start, end);
    }

    let tree = grammar(Language::Html).and_then(|g| {
        let mut parser = Parser::new();
        parser.set_language(&g).ok()?;
        parser.parse(&html, None)
    });
    match &tree {
        Some(tree) => collect(
            tree.root_node(),
            &html,
            if language == Language::Astro {
                Language::TypeScript
            } else {
                Language::Jsx
            },
            &mut blocks,
            &mut facts,
        ),
        None => {
            return FileSyntax {
                language,
                status: ParseStatus::Failed,
                errors: 1,
                error_spans: Vec::new(),
                facts: None,
                tree: None,
            };
        }
    }
    blocks.sort_by_key(|b| b.start);

    for block in blocks {
        let (start, end) = (to_u32(block.start), to_u32(block.end));
        let Some(lang) = block.language else {
            // A script in a language this engine does not read: not covered, and said so.
            errors += 1;
            error_spans.push(Span::new(start, end));
            continue;
        };
        let text = &source[block.start..block.end];
        let parsed = oxc::parse_js(text, lang, false, block.frontmatter);
        match (parsed.status, parsed.facts) {
            (ParseStatus::Failed, _) | (_, None) => {
                errors += parsed.errors.max(1);
                error_spans.push(Span::new(start, end));
            }
            (status, Some(inner)) => {
                if status == ParseStatus::ParsedWithErrors {
                    errors += parsed.errors;
                    error_spans.extend(parsed.error_spans.iter().map(|s| shift(*s, start)));
                }
                let moved =
                    |v: Vec<Span>| v.into_iter().map(|s| shift(s, start)).collect::<Vec<_>>();
                facts.comments.extend(moved(inner.comments));
                facts.strings.extend(moved(inner.strings));
                facts.regexes.extend(moved(inner.regexes));
                facts.hidden.extend(moved(inner.hidden));
            }
        }
    }

    FileSyntax {
        language,
        status: if errors == 0 {
            ParseStatus::Parsed
        } else {
            ParseStatus::ParsedWithErrors
        },
        errors,
        error_spans,
        facts: Some(facts),
        tree: None,
    }
}

/// A script of a component that the engine reads as JavaScript or TypeScript.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScriptRegion {
    /// Byte range of the code, in the file.
    pub start: usize,
    /// Byte offset after the code.
    pub end: usize,
    /// The language the script is in.
    pub language: Language,
}

/// The scripts of a Vue, Svelte or Astro file, in source order: the same blocks `parse` reads,
/// without the languages it cannot (`lang="coffee"`). Any other language gives none.
#[must_use]
pub fn script_regions(source: &str, language: Language) -> Vec<ScriptRegion> {
    if language.backend() != super::Backend::Sfc {
        return Vec::new();
    }
    let mut html = source.to_owned();
    let mut blocks = Vec::new();
    if language == Language::Astro
        && let Some((start, end)) = astro_frontmatter(source)
    {
        blocks.push(Block {
            start,
            end,
            language: Some(Language::TypeScript),
            frontmatter: true,
        });
        html = blank(source, start, end);
    }
    let tree = grammar(Language::Html).and_then(|g| {
        let mut parser = Parser::new();
        parser.set_language(&g).ok()?;
        parser.parse(&html, None)
    });
    if let Some(tree) = &tree {
        let default_lang = if language == Language::Astro {
            Language::TypeScript
        } else {
            Language::Jsx
        };
        collect(
            tree.root_node(),
            &html,
            default_lang,
            &mut blocks,
            &mut SyntaxFacts::default(),
        );
    }
    // The frontmatter is first and the HTML walk is in source order: no sorting needed.
    blocks
        .into_iter()
        .filter_map(|b| {
            Some(ScriptRegion {
                start: b.start,
                end: b.end,
                language: b.language?,
            })
        })
        .collect()
}

fn to_u32(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

fn shift(s: Span, by: u32) -> Span {
    Span::new(s.start.saturating_add(by), s.end.saturating_add(by))
}

/// The text between the two `---` lines that open an Astro file, as a byte range.
fn astro_frontmatter(source: &str) -> Option<(usize, usize)> {
    let first = source.strip_prefix("---")?;
    let rest_start = 3 + first.find('\n')? + 1;
    if !first[..rest_start - 3].trim().is_empty() {
        return None; // `---foo` is not a fence
    }
    let mut at = rest_start;
    for line in source[rest_start..].split_inclusive('\n') {
        if line.trim_end() == "---" {
            return Some((rest_start, at));
        }
        at += line.len();
    }
    None // never closed: not a frontmatter
}

/// `source` with the bytes of `start..end` replaced by spaces, newlines kept.
fn blank(source: &str, start: usize, end: usize) -> String {
    let mut out = String::with_capacity(source.len());
    for (i, ch) in source.char_indices() {
        if i >= start && i < end && ch != '\n' {
            out.extend(std::iter::repeat_n(' ', ch.len_utf8()));
        } else {
            out.push(ch);
        }
    }
    out
}

fn collect(
    node: Node<'_>,
    html: &str,
    default_lang: Language,
    blocks: &mut Vec<Block>,
    facts: &mut SyntaxFacts,
) {
    match node.kind() {
        "comment" => {
            let s = Span::new(to_u32(node.start_byte()), to_u32(node.end_byte()));
            facts.comments.push(s);
            facts.hidden.push(s);
            return;
        }
        "script_element" => {
            if let Some(block) = script_block(node, html, default_lang) {
                blocks.push(block);
            }
            return;
        }
        // Styles and templates are not parsed; a script cannot hide inside a style.
        "style_element" => return,
        _ => {}
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect(child, html, default_lang, blocks, facts);
    }
}

/// The script of a `<script>` element, or `None` when it holds no code (empty, `src=`, data).
fn script_block(node: Node<'_>, html: &str, default_lang: Language) -> Option<Block> {
    let mut cursor = node.walk();
    let children: Vec<_> = node.children(&mut cursor).collect();
    let body = children.iter().find(|c| c.kind() == "raw_text")?;
    if html[body.start_byte()..body.end_byte()].trim().is_empty() {
        return None;
    }
    let (mut lang, mut kind) = (None::<String>, None::<String>);
    if let Some(start_tag) = children.iter().find(|c| c.kind() == "start_tag") {
        let mut c = start_tag.walk();
        for attr in start_tag
            .children(&mut c)
            .filter(|a| a.kind() == "attribute")
        {
            let mut ac = attr.walk();
            let parts: Vec<_> = attr.children(&mut ac).collect();
            let name = parts.first().map(|n| &html[n.start_byte()..n.end_byte()]);
            // Children include the anonymous `=`; the value is the named node after the name.
            let value = parts
                .iter()
                .find(|v| matches!(v.kind(), "quoted_attribute_value" | "attribute_value"))
                .map(|v| {
                    html[v.start_byte()..v.end_byte()]
                        .trim_matches(|c| c == '"' || c == '\'')
                        .to_owned()
                });
            match name {
                Some("lang") => lang = value,
                Some("type") => kind = value,
                _ => {}
            }
        }
    }
    // `type="application/json"`, `text/template`, `importmap`: data, not code.
    if let Some(k) = &kind
        && !matches!(
            k.as_str(),
            "module"
                | "text/javascript"
                | "application/javascript"
                | "text/ecmascript"
                | "ts"
                | "typescript"
        )
    {
        return None;
    }
    let language = match lang.as_deref().or(kind.as_deref()) {
        None => Some(default_lang),
        Some(
            "js"
            | "javascript"
            | "module"
            | "text/javascript"
            | "application/javascript"
            | "text/ecmascript"
            | "jsx",
        ) => Some(Language::Jsx),
        Some("ts" | "typescript") => Some(Language::TypeScript),
        Some("tsx") => Some(Language::Tsx),
        Some(_) => None,
    };
    Some(Block {
        start: body.start_byte(),
        end: body.end_byte(),
        language,
        frontmatter: false,
    })
}
