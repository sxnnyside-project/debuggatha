//! Biome: JavaScript and TypeScript lint.

use super::shared::{
    Draft, Position, extension, first_prefix, is_script, located, num, scoped, strings,
};
use crate::analyzers::sarif::json_from;
use crate::analyzers::types::{Adapter, AnalyzerFinding, Invocation, RunContext, Trust};
use debuggatha_core::review_engine::Severity;
use std::path::Path;

fn biome_category(rule: &str) -> &'static str {
    first_prefix(
        &[
            (&["lint/security/"], "security"),
            (&["lint/correctness/", "lint/suspicious/"], "reliability"),
            (&["lint/performance/"], "performance"),
            (&["lint/a11y/"], "accessibility"),
        ],
        rule,
        "maintainability",
    )
}

/// `lint/suspicious/noExplicitAny` is documented at `.../rules/no-explicit-any`.
fn biome_docs(category: &str) -> String {
    let mut name = String::new();
    for c in category.rsplit('/').next().unwrap_or("").chars() {
        if c.is_ascii_uppercase() {
            name.push('-');
            name.push(c.to_ascii_lowercase());
        } else {
            name.push(c);
        }
    }
    format!("https://biomejs.dev/linter/rules/{name}")
}

fn biome_parse(stdout: &str, root: &Path) -> Result<Vec<AnalyzerFinding>, String> {
    let document = json_from(stdout)?;
    Ok(document["diagnostics"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|d| {
            let location = &d["location"];
            let file = location["path"]
                .as_str()
                .or_else(|| location["path"]["file"].as_str())?;
            let rule = d["category"].as_str().unwrap_or("biome");
            let severity = match d["severity"].as_str() {
                Some("error") => Severity::Medium,
                Some("information") => Severity::Informational,
                _ => Severity::Low,
            };
            located(
                Draft {
                    rule_id: rule.to_owned(),
                    message: d["message"].as_str().unwrap_or(rule).to_owned(),
                    severity,
                    category: biome_category(rule),
                    url: rule.starts_with("lint/").then(|| biome_docs(rule)),
                },
                root,
                file,
                Position {
                    line: num(&location["start"]["line"]),
                    column: num(&location["start"]["column"]),
                    end_line: num(&location["end"]["line"]),
                    end_column: num(&location["end"]["column"]),
                },
            )
        })
        .collect())
}

fn biome_command(context: &RunContext<'_>) -> Invocation {
    let mut args = strings(&[
        "lint",
        "--reporter=json",
        "--colors=off",
        "--max-diagnostics=none",
    ]);
    args.extend(scoped(context, &["."]));
    Invocation {
        args,
        cleanup: None,
    }
}

/// Biome, the JavaScript and TypeScript linter.
pub static BIOME: Adapter = Adapter {
    id: "biome",
    name: "Biome",
    license: "MIT OR Apache-2.0",
    homepage: "https://biomejs.dev",
    covers: "JavaScript, TypeScript, JSON, CSS",
    trust: Trust::Safe,
    applies: |facts| facts.has_file_named(&["biome.json", "biome.jsonc"]),
    local_binaries: &["node_modules/.bin/biome"],
    binary_names: &["biome"],
    prefix: &[],
    version_args: &["--version"],
    command: biome_command,
    ok_exit_codes: &[0, 1],
    parse: biome_parse,
    supersedes: &[],
    equivalent_to: &[
        ("lint/suspicious/noDoubleEquals", "eqeqeq"),
        ("lint/suspicious/noExplicitAny", "no-explicit-any"),
        ("lint/security/noGlobalEval", "no-eval"),
        ("lint/style/noVar", "no-var"),
        ("lint/correctness/noVar", "no-var"),
    ],
    file_filter: Some(|f| is_script(f) || matches!(extension(f), "json" | "css")),
};
