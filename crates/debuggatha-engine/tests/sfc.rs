//! Single-file components: script blocks are found with the HTML grammar, parsed by oxc, and their
//! facts come back at offsets in the file (so lines, columns and suppression comments are right).
#![allow(clippy::expect_used, clippy::unwrap_used)] // a failed expect is a failed test

use std::path::Path;

use debuggatha_engine::syntax::{
    FileSyntax, Language, ParseStatus, Query, QueryError, Span, comment_lines, find_directive,
    mask_source, parse, parse_directives,
};

fn p(file: &str, src: &str) -> FileSyntax {
    parse(Path::new(file), src).expect("a supported language")
}

fn masked(file: &str, src: &str) -> String {
    mask_source(src, &p(file, src).facts.expect("facts"))
}

#[test]
fn vue_script_is_masked_in_place_and_the_template_is_left_alone() {
    let src = "<script setup lang=\"ts\">\nconst k: string = 'secret'; // note\n</script>\n\n<template>\n  <p>{{ \"shown\" }}</p>\n</template>\n";
    let s = p("A.vue", src);
    assert_eq!(s.status, ParseStatus::Parsed);
    assert_eq!(
        masked("A.vue", src),
        "<script setup lang=\"ts\">\nconst k: string = '      ';        \n</script>\n\n<template>\n  <p>{{ \"shown\" }}</p>\n</template>\n"
    );
    assert!(
        Language::Vue.limited(),
        "templates are not parsed, and the coverage must say so"
    );
}

#[test]
fn two_scripts_in_one_component_both_count() {
    let src = "<script>\nexport const a = 'one';\n</script>\n<script setup>\nconst b = 'two';\n</script>\n";
    let f = p("A.vue", src).facts.unwrap();
    assert_eq!(f.strings.len(), 2);
    let texts: Vec<_> = f
        .strings
        .iter()
        .map(|s| &src[s.start as usize..s.end as usize])
        .collect();
    assert_eq!(texts, ["'one'", "'two'"]);
}

#[test]
fn directives_keep_their_file_line_in_scripts_and_in_the_template() {
    let src = "<script lang=\"ts\">\n// debuggatha-ignore-next-line no-eval -- loader\neval(x);\n</script>\n\n<!-- debuggatha-ignore-next-line no-v-html -- sanitized -->\n<div v-html=\"h\"></div>\n";
    let s = p("A.svelte", src);
    let directives = parse_directives(&comment_lines(src, &s.facts.unwrap()));
    assert_eq!(directives.len(), 2);
    assert!(
        find_directive(&directives, 3, "no-eval").is_some(),
        "eval( is on line 3 of the file"
    );
    assert!(
        find_directive(&directives, 7, "no-v-html").is_some(),
        "the div is on line 7"
    );
    assert!(find_directive(&directives, 7, "no-eval").is_none());
}

#[test]
fn astro_frontmatter_is_typescript_and_a_script_in_its_string_is_not_a_block() {
    let src = "---\nconst html: string = \"<script>bad(</script>\";\nconst e = import.meta.env.X; // fm\n---\n<h1>{e}</h1>\n<script>\n  const c = 'client';\n</script>\n";
    let s = p("a.astro", src);
    assert_eq!(s.status, ParseStatus::Parsed, "{:?}", s.error_spans);
    let f = s.facts.unwrap();
    let texts: Vec<_> = f
        .strings
        .iter()
        .map(|s| &src[s.start as usize..s.end as usize])
        .collect();
    assert_eq!(texts, ["\"<script>bad(</script>\"", "'client'"]);
    assert_eq!(f.comments.len(), 1);
}

#[test]
fn a_block_that_cannot_be_read_is_an_error_region_and_the_rest_survives() {
    let src =
        "<script>\nconst a = `unterminated\n</script>\n<script>\nconst ok = 'fine';\n</script>\n";
    let s = p("A.svelte", src);
    assert_eq!(s.status, ParseStatus::ParsedWithErrors);
    let bad_start = u32::try_from(src.find("\nconst a").unwrap()).unwrap();
    assert!(s.touches_error(Span::new(bad_start + 1, bad_start + 5)));
    let ok_at = u32::try_from(src.find("'fine'").unwrap()).unwrap();
    assert!(!s.touches_error(Span::new(ok_at, ok_at + 6)));
    assert_eq!(
        s.facts.unwrap().strings.len(),
        1,
        "the intact block keeps its strings"
    );
}

#[test]
fn a_script_in_another_language_is_not_covered_and_says_so() {
    let src = "<script lang=\"coffee\">\nx = 1\n</script>\n";
    let s = p("A.vue", src);
    assert_eq!(s.status, ParseStatus::ParsedWithErrors);
    assert_eq!(s.errors, 1);
}

#[test]
fn data_scripts_styles_and_empty_scripts_are_not_code() {
    let src = "<script type=\"application/json\">{\"a\": \"'\"}</script>\n<script src=\"x.js\"></script>\n<style>\n/* css */ a { content: '\"'; }\n</style>\n<p>hi</p>\n";
    let s = p("A.svelte", src);
    assert_eq!(s.status, ParseStatus::Parsed);
    let f = s.facts.unwrap();
    assert!(f.comments.is_empty() && f.strings.is_empty());
}

#[test]
fn offsets_are_right_after_multibyte_text_and_crlf() {
    let src = "<p>日本語 🚀</p>\r\n<script>\r\nconst s = 'é'; // ü\r\n</script>\r\n";
    let s = p("A.vue", src);
    assert_eq!(s.status, ParseStatus::Parsed);
    let f = s.facts.unwrap();
    let c = f.comments[0];
    assert_eq!(&src[c.start as usize..c.end as usize], "// ü");
    let lines = comment_lines(src, &f);
    assert_eq!((lines[0].line, lines[0].text.as_str()), (3, " ü"));
}

#[test]
fn a_template_only_component_parses_clean_and_has_no_facts() {
    let s = p("A.vue", "<template><p>{{ x }}</p></template>\n");
    assert_eq!(s.status, ParseStatus::Parsed);
    let f = s.facts.unwrap();
    assert!(f.strings.is_empty() && f.comments.is_empty());
}

#[test]
fn queries_do_not_run_on_a_component() {
    assert!(matches!(
        Query::new(Language::Vue, "(identifier) @x"),
        Err(QueryError::NotTreeSitter(_))
    ));
}

#[test]
fn astro_reads_scripts_as_typescript_and_allows_return_in_the_frontmatter() {
    let src = "---\nif (!Astro.props.id) return Astro.redirect('/404');\n---\n<script>\n  const el = document.querySelector('p')!;\n  type T = string;\n</script>\n";
    let s = p("a.astro", src);
    assert_eq!(s.status, ParseStatus::Parsed, "{:?}", s.error_spans);
    // The same TypeScript in a Svelte script without `lang` is JavaScript, and does not parse.
    let svelte = "<script>\n  const el = document.querySelector('p')!;\n</script>\n";
    assert_eq!(p("a.svelte", svelte).status, ParseStatus::ParsedWithErrors);
}
