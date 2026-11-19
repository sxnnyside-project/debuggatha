//! `ESLint`: JavaScript and TypeScript lint that runs the project's own config.

use super::shared::{
    Draft, Position, base_name, first_prefix, is_script, located, num, scoped, strings,
};
use crate::analyzers::types::{Adapter, AnalyzerFinding, Invocation, RepoFacts, RunContext, Trust};
use debuggatha_core::review_engine::Severity;
use serde_json::Value;
use std::path::Path;

fn eslint_category(rule: &str) -> &'static str {
    first_prefix(
        &[
            (
                &[
                    "no-eval",
                    "no-implied-eval",
                    "no-new-func",
                    "no-script-url",
                    "security/",
                ],
                "security",
            ),
            (
                &[
                    "eqeqeq",
                    "no-undef",
                    "no-unused-vars",
                    "no-dupe",
                    "no-unreachable",
                    "no-cond-assign",
                    "use-isnan",
                    "valid-typeof",
                    "react-hooks/",
                ],
                "reliability",
            ),
            (&["jsx-a11y/"], "accessibility"),
        ],
        rule,
        "maintainability",
    )
}

fn eslint_parse(stdout: &str, root: &Path) -> Result<Vec<AnalyzerFinding>, String> {
    let start = stdout.find('[').ok_or("no JSON document in the output")?;
    let files: Value = serde_json::from_str(&stdout[start..]).map_err(|e| e.to_string())?;
    Ok(files
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|entry| {
            let file = entry["filePath"].as_str().unwrap_or("");
            entry["messages"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(move |m| {
                    let original = m["ruleId"].as_str();
                    let rule = original.unwrap_or("parse-error");
                    located(
                        Draft {
                            rule_id: rule.to_owned(),
                            message: m["message"].as_str().unwrap_or(rule).to_owned(),
                            severity: if m["severity"].as_u64().unwrap_or(0) >= 2 {
                                Severity::Medium
                            } else {
                                Severity::Low
                            },
                            category: eslint_category(rule),
                            url: original
                                .filter(|r| !r.contains('/'))
                                .map(|r| format!("https://eslint.org/docs/latest/rules/{r}")),
                        },
                        root,
                        file,
                        Position {
                            line: num(&m["line"]),
                            column: num(&m["column"]),
                            end_line: num(&m["endLine"]),
                            end_column: num(&m["endColumn"]),
                        },
                    )
                })
        })
        .collect())
}

fn eslint_command(context: &RunContext<'_>) -> Invocation {
    let mut args = strings(&[
        "--format",
        "json",
        "--no-warn-ignored",
        "--no-error-on-unmatched-pattern",
    ]);
    args.extend(scoped(context, &["."]));
    Invocation {
        args,
        cleanup: None,
    }
}

fn eslint_applies(facts: &RepoFacts) -> bool {
    facts.any(|f| {
        let name = base_name(f);
        let config = name
            .strip_prefix("eslint.config.")
            .is_some_and(|ext| matches!(ext, "js" | "ts" | "cjs" | "mjs" | "cts" | "mts"));
        let legacy = name == ".eslintrc"
            || name
                .strip_prefix(".eslintrc.")
                .is_some_and(|ext| !ext.is_empty() && ext.bytes().all(|b| b.is_ascii_lowercase()));
        config || legacy
    })
}

/// `ESLint` with the project's own configuration, which can load project code.
pub static ESLINT: Adapter = Adapter {
    id: "eslint",
    name: "ESLint",
    license: "MIT",
    homepage: "https://eslint.org",
    covers: "JavaScript, TypeScript",
    // An ESLint config is JavaScript, and plugins are packages the project chose to run.
    trust: Trust::RunsProjectCode,
    applies: eslint_applies,
    local_binaries: &["node_modules/.bin/eslint"],
    binary_names: &["eslint"],
    prefix: &[],
    version_args: &["--version"],
    command: eslint_command,
    ok_exit_codes: &[0, 1],
    parse: eslint_parse,
    supersedes: &[],
    equivalent_to: &[
        ("no-var", "no-var"),
        ("eqeqeq", "eqeqeq"),
        ("no-eval", "no-eval"),
        ("no-implied-eval", "no-implied-eval"),
        ("no-console", "avoid-print"),
    ],
    file_filter: Some(is_script),
};
