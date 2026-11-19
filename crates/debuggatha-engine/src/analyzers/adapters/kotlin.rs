//! ktlint and detekt: Kotlin style and static analysis.

use super::shared::{base_name, extension, first_prefix, scoped, strings};
use crate::analyzers::sarif::{SarifOptions, parse_sarif, severity_from_level};
use crate::analyzers::types::{Adapter, Invocation, RepoFacts, RunContext, Trust};
use debuggatha_core::review_engine::Severity;

fn is_kotlin(file: &str) -> bool {
    matches!(extension(file), "kt" | "kts")
}

fn ktlint_command(context: &RunContext<'_>) -> Invocation {
    let mut args = strings(&["--reporter=sarif"]);
    args.extend(scoped(
        context,
        &["**/*.kt", "**/*.kts", "!**/build/**", "!**/.gradle/**"],
    ));
    Invocation {
        args,
        cleanup: None,
    }
}

/// ktlint, the Kotlin style checker.
pub static KTLINT: Adapter = Adapter {
    id: "ktlint",
    name: "ktlint",
    license: "MIT",
    homepage: "https://pinterest.github.io/ktlint",
    covers: "Kotlin style",
    trust: Trust::Safe,
    applies: |facts| facts.any(is_kotlin),
    local_binaries: &[],
    binary_names: &["ktlint"],
    prefix: &[],
    version_args: &["--version"],
    command: ktlint_command,
    ok_exit_codes: &[0, 1],
    parse: |stdout, root| {
        parse_sarif(
            stdout,
            root,
            &SarifOptions {
                category: |_| "maintainability",
                severity: Some(|_| Severity::Low),
                sensitive: false,
            },
        )
    },
    supersedes: &[],
    equivalent_to: &[],
    file_filter: Some(is_kotlin),
};

const DETEKT_CONFIGS: &[&str] = &[
    "detekt.yml",
    "config/detekt/detekt.yml",
    "config/detekt.yml",
    "detekt-config.yml",
];

fn detekt_command(context: &RunContext<'_>) -> Invocation {
    let input = context
        .files
        .map_or_else(|| ".".to_owned(), |f| f.join(","));
    let mut args = vec!["--input".to_owned(), input];
    if let Some(config) = DETEKT_CONFIGS
        .iter()
        .find(|name| context.root.join(name).exists())
    {
        args.extend(strings(&[
            "--config",
            config,
            "--build-upon-default-config",
        ]));
    }
    args.extend(strings(&["--report", "sarif:/dev/stdout"]));
    Invocation {
        args,
        cleanup: None,
    }
}

fn detekt_applies(facts: &RepoFacts) -> bool {
    // Without a config detekt would judge the project by its defaults: the repository has to have
    // chosen it.
    facts.any(is_kotlin)
        && facts.any(|f| {
            let name = base_name(f);
            name.strip_prefix("detekt").is_some_and(|rest| {
                rest.strip_suffix(".yml")
                    .or_else(|| rest.strip_suffix(".yaml"))
                    .is_some_and(|mid| {
                        mid.bytes()
                            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-'))
                    })
            })
        })
}

/// detekt, the Kotlin static analyzer.
pub static DETEKT: Adapter = Adapter {
    id: "detekt",
    name: "detekt",
    license: "Apache-2.0",
    homepage: "https://detekt.dev",
    covers: "Kotlin code smells and bugs",
    trust: Trust::Safe,
    applies: detekt_applies,
    local_binaries: &[],
    binary_names: &["detekt-cli", "detekt"],
    prefix: &[],
    version_args: &["--version"],
    command: detekt_command,
    // 2: issues found. 0 and 2 both mean the report is valid.
    ok_exit_codes: &[0, 2],
    parse: |stdout, root| {
        parse_sarif(
            stdout,
            root,
            &SarifOptions {
                category: |rule| {
                    first_prefix(
                        &[
                            (
                                &["detekt.potential-bugs.", "detekt.coroutines."],
                                "reliability",
                            ),
                            (&["detekt.performance."], "performance"),
                        ],
                        rule,
                        "maintainability",
                    )
                },
                severity: Some(|level| {
                    severity_from_level(Some(if level == Some("error") {
                        "error"
                    } else {
                        "warning"
                    }))
                }),
                sensitive: false,
            },
        )
    },
    supersedes: &[],
    equivalent_to: &[],
    file_filter: Some(is_kotlin),
};
