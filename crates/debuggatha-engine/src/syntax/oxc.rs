//! The oxc backend for the JavaScript and TypeScript family. Kept behind `parse_js`: oxc is
//! pre-1.0 and its types never leave this file (ADR-0006, decision 4).

use oxc_allocator::Allocator;
use oxc_ast::ast::{RegExpLiteral, StringLiteral, TemplateElement, TemplateLiteral};
use oxc_ast_visit::{Visit, walk};
use oxc_parser::{ParseOptions, Parser};
use oxc_span::SourceType;

use super::language::Language;
use super::{FileSyntax, ParseStatus, Span, SyntaxFacts};

pub(crate) fn source_type(language: Language, declaration: bool) -> SourceType {
    match language {
        Language::TypeScript if declaration => SourceType::d_ts(),
        Language::JavaScript => SourceType::mjs(),
        Language::Jsx => SourceType::jsx(),
        Language::TypeScript => SourceType::ts(),
        // Not a JS/TS language: the dispatcher never sends one here.
        _ => SourceType::tsx(),
    }
}

struct Collector<'s> {
    source: &'s str,
    strings: Vec<Span>,
    regexes: Vec<Span>,
    hidden: Vec<Span>,
}

impl<'a> Visit<'a> for Collector<'_> {
    // The quotes stay; what is between them is hidden.
    fn visit_string_literal(&mut self, it: &StringLiteral<'a>) {
        self.strings.push(Span::new(it.span.start, it.span.end));
        if it.span.end >= it.span.start + 2 {
            self.hidden
                .push(Span::new(it.span.start + 1, it.span.end - 1));
        }
    }

    // The literal is a string with its backticks; only its text parts are hidden, `${...}` is code.
    fn visit_template_literal(&mut self, it: &TemplateLiteral<'a>) {
        self.strings.push(Span::new(it.span.start, it.span.end));
        walk::walk_template_literal(self, it);
    }

    fn visit_template_element(&mut self, it: &TemplateElement<'a>) {
        self.hidden.push(Span::new(it.span.start, it.span.end));
    }

    // The pattern is hidden, the slashes and the flags stay.
    fn visit_reg_exp_literal(&mut self, it: &RegExpLiteral<'a>) {
        self.regexes.push(Span::new(it.span.start, it.span.end));
        let text = self
            .source
            .get(it.span.start as usize..it.span.end as usize)
            .unwrap_or("");
        if let Some(close) = text.rfind('/').filter(|c| *c > 0) {
            let close = u32::try_from(close).unwrap_or(0);
            self.hidden
                .push(Span::new(it.span.start + 1, it.span.start + close));
        }
    }
}

/// `declaration` is a `.d.ts`-style file, where ambient forms are legal that an implementation file
/// would reject. `top_level_return` is for a component frontmatter, where `return` is legal.
pub(super) fn parse_js(
    source: &str,
    language: Language,
    declaration: bool,
    top_level_return: bool,
) -> FileSyntax {
    let allocator = Allocator::default();
    let options = ParseOptions {
        allow_return_outside_function: top_level_return,
        ..ParseOptions::default()
    };
    let ret = Parser::new(&allocator, source, source_type(language, declaration))
        .with_options(options)
        .parse();
    let errors = ret.diagnostics.errors().count();

    if ret.fatal_error {
        // A fatal error: the tree is empty, so nothing is concluded about this file.
        return FileSyntax {
            language,
            status: ParseStatus::Failed,
            errors: errors.max(1),
            error_spans: Vec::new(),
            facts: None,
            tree: None,
        };
    }

    let error_spans = ret
        .diagnostics
        .errors()
        .flat_map(|d| d.labels.iter())
        .map(|l| Span::new(l.offset(), l.offset() + l.len()))
        .collect();

    let comments: Vec<Span> = ret
        .program
        .comments
        .iter()
        .map(|c| Span::new(c.span.start, c.span.end))
        .collect();
    let mut collector = Collector {
        source,
        strings: Vec::new(),
        regexes: Vec::new(),
        hidden: Vec::new(),
    };
    collector.visit_program(&ret.program);

    FileSyntax {
        language,
        status: if errors == 0 {
            ParseStatus::Parsed
        } else {
            ParseStatus::ParsedWithErrors
        },
        errors,
        error_spans,
        tree: None,
        facts: Some(SyntaxFacts {
            comments: comments.clone(),
            strings: collector.strings,
            regexes: collector.regexes,
            hidden: comments.into_iter().chain(collector.hidden).collect(),
        }),
    }
}
