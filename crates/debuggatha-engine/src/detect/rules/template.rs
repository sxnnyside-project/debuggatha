//! Group D: rules about the template of a single-file component. `use-v-html-carefully` reads the
//! attributes of the HTML grammar instead of searching text, so `:v-html`, `data-v-html` and a
//! `v-html` inside a comment are not the directive.
//!
//! The component is parsed again as HTML (the SFC parse keeps only the script blocks); a template
//! is HTML plus directives, and the grammar is lenient with the directive syntax.

use std::path::Path;
use std::sync::OnceLock;

use super::calls::one_per_line;
use super::js::{Lines, make_hit};
use crate::detect::hit::Hit;
use crate::detect::input::DetectorInput;
use crate::syntax::{Language, Query, parse};
use debuggatha_core::review_engine::Confidence;

use crate::detect::runner::Registry;

/// The rules this group answers for.
pub const RULES: &[&str] = &["use-v-html-carefully"];

/// Registers the group.
pub fn register(registry: &mut Registry) {
    registry.register_group(RULES, detect);
}

const SOURCE: &str = include_str!("../../../data/queries/vue-template.scm");

fn query() -> Result<&'static Query, &'static str> {
    static QUERY: OnceLock<Result<Query, String>> = OnceLock::new();
    QUERY
        .get_or_init(|| Query::new(Language::Html, SOURCE).map_err(|e| e.to_string()))
        .as_ref()
        .map_err(String::as_str)
}

/// Runs the group over one file.
#[must_use]
pub fn detect(input: &DetectorInput<'_>) -> Vec<(&'static str, Hit)> {
    if input.syntax.map(|s| s.language) != Some(Language::Vue) {
        return Vec::new();
    }
    let (Ok(query), Some(html)) = (query(), parse(Path::new("template.html"), input.content))
    else {
        return Vec::new();
    };
    let Ok(matches) = query.run(&html, input.content) else {
        return Vec::new();
    };
    // Errors are those of the HTML parse: the template is what is being read.
    let html_input = DetectorInput::new(input.file, input.content, Some(&html));
    let lines = Lines::new(input.content);
    let mut hits = Vec::new();
    for m in matches {
        let Some(name) = m.captures.iter().find(|c| c.name == "name").map(|c| c.span) else {
            continue;
        };
        // The old pattern was `v-html`, spaces, `=`: an attribute with no value is not it.
        let after = &input.content[name.end as usize..];
        let rest = after.trim_start();
        if !rest.starts_with('=') {
            continue;
        }
        let end = name.end as usize + (after.len() - rest.len()) + 1;
        if let Some(hit) = make_hit(
            &html_input,
            &lines,
            name.start as usize,
            end,
            Confidence::High,
            None,
        ) {
            hits.push(("use-v-html-carefully", hit, name.start as usize));
        }
    }
    one_per_line(hits)
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_query_compiles() {
        assert_eq!(super::query().err(), None);
    }
}
