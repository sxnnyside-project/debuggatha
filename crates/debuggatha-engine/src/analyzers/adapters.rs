//! One adapter per tool. Each runs the tool as its own process and reads what it prints; none
//! bundles, copies or redistributes the tool or its rules. The commands and parsers were checked
//! against the real tools' recorded output (`tests/fixtures/analyzers`).

use std::path::Path;
use std::sync::atomic::{AtomicU32, Ordering};

use debuggatha_core::findings_ledger::sha256_hex;
use debuggatha_core::review_engine::Severity;
use serde_json::Value;

use super::paths::to_repository_path;
use super::sarif::{SarifOptions, json_from, parse_sarif, severity_from_level};
use super::types::{Adapter, AnalyzerFinding, Invocation, RepoFacts, RunContext, Trust};
use crate::scan_scope::IGNORED_DIRS;

/// What a tool says about one finding before its file is checked against the repository.
struct Draft {
    rule_id: String,
    message: String,
    severity: Severity,
    category: &'static str,
    url: Option<String>,
}

#[derive(Default, Clone, Copy)]
struct Position {
    line: Option<u32>,
    column: Option<u32>,
    end_line: Option<u32>,
    end_column: Option<u32>,
}

/// `None` when the file is outside the repository.
fn located(draft: Draft, root: &Path, file: &str, at: Position) -> Option<AnalyzerFinding> {
    Some(AnalyzerFinding {
        rule_id: draft.rule_id,
        message: draft.message,
        severity: draft.severity,
        category: draft.category.to_owned(),
        file: to_repository_path(root, file)?,
        line: at.line,
        column: at.column,
        end_line: at.end_line,
        end_column: at.end_column,
        url: draft.url,
        sensitive: false,
    })
}

fn num(value: &Value) -> Option<u32> {
    value.as_u64().and_then(|n| u32::try_from(n).ok())
}

fn scoped(context: &RunContext<'_>, fallback: &[&str]) -> Vec<String> {
    context.files.map_or_else(
        || fallback.iter().map(|s| (*s).to_owned()).collect(),
        <[String]>::to_vec,
    )
}

fn strings(args: &[&str]) -> Vec<String> {
    args.iter().map(|a| (*a).to_owned()).collect()
}

fn first_prefix(
    table: &[(&[&str], &'static str)],
    id: &str,
    fallback: &'static str,
) -> &'static str {
    table
        .iter()
        .find(|(prefixes, _)| prefixes.iter().any(|p| id.starts_with(p)))
        .map_or(fallback, |(_, category)| category)
}

fn extension(file: &str) -> &str {
    file.rsplit_once('.').map_or("", |(_, e)| e)
}

fn is_script(file: &str) -> bool {
    // `.js .jsx .ts .tsx`, each with an optional `m` or `c` before it.
    let ext = extension(file);
    let ext = ext.strip_prefix(['m', 'c']).unwrap_or(ext);
    matches!(ext, "js" | "jsx" | "ts" | "tsx")
}

fn base_name(file: &str) -> &str {
    file.rsplit('/').next().unwrap_or(file)
}

/* ------------------------------------------------------------------ gitleaks */

/// Makes the name of a generated config directory unique within the process.
static COUNTER: AtomicU32 = AtomicU32::new(0);

fn gitleaks_command(context: &RunContext<'_>) -> Invocation {
    let mut args: Vec<String> = [
        "dir",
        ".",
        "--no-banner",
        "--log-level",
        "error",
        "--redact",
        "--report-format",
        "sarif",
        "--report-path",
        "-",
        "--exit-code",
        "0",
    ]
    .map(str::to_owned)
    .to_vec();
    if [".gitleaks.toml", "gitleaks.toml"]
        .iter()
        .any(|name| context.root.join(name).exists())
    {
        return Invocation {
            args,
            cleanup: None,
        };
    }
    // No repository config: scan what the project owns. `dir` does not read `.gitignore`.
    let directory = std::env::temp_dir().join(format!(
        "debuggatha-gitleaks-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    let skipped: Vec<String> = IGNORED_DIRS.iter().map(|d| escape_regex(d)).collect();
    let config = directory.join("gitleaks.toml");
    let text = format!(
        "[extend]\nuseDefault = true\n\n[[allowlists]]\npaths = ['''(^|/)({})/''']\n",
        skipped.join("|")
    );
    if std::fs::create_dir_all(&directory)
        .and_then(|()| std::fs::write(&config, text))
        .is_ok()
    {
        args.extend(["--config".to_owned(), config.to_string_lossy().into_owned()]);
        return Invocation {
            args,
            cleanup: Some(directory),
        };
    }
    Invocation {
        args,
        cleanup: None,
    }
}

fn escape_regex(text: &str) -> String {
    let mut out = String::new();
    for c in text.chars() {
        if ".\\+*?()|[]{}^$".contains(c) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

fn gitleaks_parse(stdout: &str, root: &Path) -> Result<Vec<AnalyzerFinding>, String> {
    parse_sarif(
        stdout,
        root,
        &SarifOptions {
            category: |_| "security",
            severity: Some(|_| Severity::Critical),
            sensitive: true,
        },
    )
}

pub static GITLEAKS: Adapter = Adapter {
    id: "gitleaks",
    name: "Gitleaks",
    license: "MIT",
    homepage: "https://github.com/gitleaks/gitleaks",
    covers: "secrets in any language",
    trust: Trust::Safe,
    applies: |_| true,
    local_binaries: &[],
    binary_names: &["gitleaks"],
    prefix: &[],
    version_args: &["version"],
    command: gitleaks_command,
    ok_exit_codes: &[0],
    parse: gitleaks_parse,
    supersedes: &["no-hardcoded-secrets"],
    equivalent_to: &[],
    file_filter: None,
};

fn osv_applies(facts: &RepoFacts) -> bool {
    facts.has_file_named(&[
        "package-lock.json",
        "yarn.lock",
        "pnpm-lock.yaml",
        "bun.lock",
        "Cargo.lock",
        "composer.lock",
        "go.sum",
        "Gemfile.lock",
        "poetry.lock",
        "uv.lock",
        "requirements.txt",
        "gradle.lockfile",
        "pom.xml",
    ])
}

fn osv_parse(stdout: &str, root: &Path) -> Result<Vec<AnalyzerFinding>, String> {
    if stdout.trim().is_empty() {
        return Ok(Vec::new());
    }
    parse_sarif(
        stdout,
        root,
        &SarifOptions {
            category: |_| "security",
            severity: None,
            sensitive: false,
        },
    )
}

pub static OSV_SCANNER: Adapter = Adapter {
    id: "osv-scanner",
    name: "OSV-Scanner",
    license: "Apache-2.0",
    homepage: "https://google.github.io/osv-scanner",
    covers: "known vulnerabilities in dependencies",
    // It sends package names and versions to the OSV.dev API.
    trust: Trust::Network,
    applies: osv_applies,
    local_binaries: &[],
    binary_names: &["osv-scanner"],
    prefix: &[],
    version_args: &["--version"],
    command: |_| Invocation {
        args: ["scan", "source", "--format", "sarif", "--recursive", "."]
            .map(str::to_owned)
            .to_vec(),
        cleanup: None,
    },
    // 1: vulnerabilities found. 128: nothing to scan.
    ok_exit_codes: &[0, 1, 128],
    parse: osv_parse,
    supersedes: &[],
    equivalent_to: &[],
    file_filter: None,
};

/* --------------------------------------------------------------------- biome */

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

/* -------------------------------------------------------------------- eslint */

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

/* ---------------------------------------------------------------------- ruff */

fn ruff_parse(stdout: &str, root: &Path) -> Result<Vec<AnalyzerFinding>, String> {
    let diagnostics: Value = serde_json::from_str(stdout).map_err(|e| e.to_string())?;
    Ok(diagnostics
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|d| {
            let original = d["code"].as_str();
            let code = original.unwrap_or("syntax-error");
            let flaw = ["F", "E9", "B"].iter().any(|p| code.starts_with(p));
            let security = code.starts_with('S');
            located(
                Draft {
                    rule_id: code.to_owned(),
                    message: d["message"].as_str().unwrap_or(code).to_owned(),
                    severity: if flaw || security || original.is_none() {
                        Severity::Medium
                    } else {
                        Severity::Low
                    },
                    category: if security {
                        "security"
                    } else if flaw || original.is_none() {
                        "reliability"
                    } else {
                        "maintainability"
                    },
                    url: d["url"].as_str().map(str::to_owned),
                },
                root,
                d["filename"].as_str()?,
                Position {
                    line: num(&d["location"]["row"]),
                    column: num(&d["location"]["column"]),
                    end_line: num(&d["end_location"]["row"]),
                    end_column: num(&d["end_location"]["column"]),
                },
            )
        })
        .collect())
}

fn ruff_command(context: &RunContext<'_>) -> Invocation {
    // `--no-cache` keeps Ruff from writing `.ruff_cache` into the repository.
    let mut args = strings(&[
        "check",
        "--output-format",
        "json",
        "--exit-zero",
        "--no-cache",
    ]);
    args.extend(scoped(context, &["."]));
    Invocation {
        args,
        cleanup: None,
    }
}

pub static RUFF: Adapter = Adapter {
    id: "ruff",
    name: "Ruff",
    license: "MIT",
    homepage: "https://docs.astral.sh/ruff",
    covers: "Python",
    trust: Trust::Safe,
    applies: |facts| facts.any(|f| matches!(extension(f), "py" | "pyi")),
    local_binaries: &[".venv/bin/ruff", "venv/bin/ruff"],
    binary_names: &["ruff"],
    prefix: &[],
    version_args: &["--version"],
    command: ruff_command,
    ok_exit_codes: &[0],
    parse: ruff_parse,
    supersedes: &[],
    equivalent_to: &[],
    file_filter: Some(|f| matches!(extension(f), "py" | "pyi")),
};

/* ----------------------------------------------------------- ktlint, detekt */

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

/* ------------------------------------------------------------------- phpstan */

const PHPSTAN_CONFIGS: &[&str] = &["phpstan.neon", "phpstan.neon.dist", "phpstan.dist.neon"];
const PHP_SOURCE_DIRS: &[&str] = &["src", "app", "lib"];

fn phpstan_command(context: &RunContext<'_>) -> Invocation {
    let configured = PHPSTAN_CONFIGS
        .iter()
        .any(|name| context.root.join(name).exists());
    let directories: Vec<String> = PHP_SOURCE_DIRS
        .iter()
        .filter(|name| context.root.join(name).exists())
        .map(|name| (*name).to_owned())
        .collect();
    let mut args = strings(&[
        "analyse",
        "--error-format=json",
        "--no-progress",
        "--no-interaction",
        // PHP's own default of 128M is too little for a real application.
        "--memory-limit=1G",
    ]);
    // Without a config there is no level; 0 only reports what is certainly wrong.
    if !configured {
        args.push("--level=0".to_owned());
    }
    match context.files {
        Some(files) => args.extend(files.iter().cloned()),
        None if configured => {}
        None if directories.is_empty() => args.push(".".to_owned()),
        None => args.extend(directories),
    }
    Invocation {
        args,
        cleanup: None,
    }
}

fn phpstan_parse(stdout: &str, root: &Path) -> Result<Vec<AnalyzerFinding>, String> {
    let report = json_from(stdout)?;
    // A run that did not finish says nothing about the code; "no findings" would be a lie.
    let problems = report["errors"]
        .as_array()
        .into_iter()
        .chain(report["general_errors"].as_array())
        .flatten()
        .find_map(Value::as_str);
    if let Some(problem) = problems {
        let first = problem.lines().next().unwrap_or("");
        return Err(format!("PHPStan could not finish: {first}"));
    }
    if report["result"].as_str() == Some("failed") && report.get("files").is_none() {
        return Err("PHPStan reported a failed run without saying why".to_owned());
    }
    Ok(report["files"]
        .as_object()
        .into_iter()
        .flatten()
        .flat_map(|(file, entry)| {
            entry["messages"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(move |m| {
                    let identifier = m["identifier"].as_str();
                    located(
                        Draft {
                            rule_id: identifier.unwrap_or("phpstan").to_owned(),
                            message: m["message"].as_str().unwrap_or("").to_owned(),
                            severity: Severity::Medium,
                            category: "reliability",
                            url: identifier
                                .map(|i| format!("https://phpstan.org/error-identifiers/{i}")),
                        },
                        root,
                        file,
                        Position {
                            line: num(&m["line"]).filter(|l| *l > 0),
                            ..Position::default()
                        },
                    )
                })
        })
        .collect())
}

fn phpstan_applies(facts: &RepoFacts) -> bool {
    facts.any(|f| extension(f) == "php")
        && facts.has_file_named(&[
            "composer.json",
            "phpstan.neon",
            "phpstan.dist.neon",
            "phpstan.neon.dist",
            "phpstan.dist.neon.dist",
        ])
}

pub static PHPSTAN: Adapter = Adapter {
    id: "phpstan",
    name: "PHPStan",
    license: "MIT",
    homepage: "https://phpstan.org",
    covers: "PHP type and logic errors",
    // PHPStan loads the project's autoloader and bootstrap files.
    trust: Trust::RunsProjectCode,
    applies: phpstan_applies,
    local_binaries: &["vendor/bin/phpstan"],
    binary_names: &["phpstan"],
    prefix: &[],
    version_args: &["--version"],
    command: phpstan_command,
    ok_exit_codes: &[0, 1],
    parse: phpstan_parse,
    supersedes: &[],
    equivalent_to: &[],
    file_filter: Some(|f| extension(f) == "php"),
};

/* ------------------------------------------------------------------- clippy */

fn clippy_category(rule: &str) -> &'static str {
    const RELIABILITY: &[&str] = &[
        "unwrap_used",
        "expect_used",
        "panic",
        "indexing_slicing",
        "unreachable",
        "todo",
        "unimplemented",
    ];
    if RELIABILITY.iter().any(|p| rule.contains(p)) {
        "reliability"
    } else if [
        "clippy::perf",
        "clippy::redundant_clone",
        "clippy::useless_vec",
        "clippy::needless_collect",
    ]
    .iter()
    .any(|p| rule.starts_with(p))
    {
        "performance"
    } else {
        "maintainability"
    }
}

#[allow(clippy::unnecessary_wraps)] // the shape every adapter's `parse` has
fn clippy_parse(stdout: &str, root: &Path) -> Result<Vec<AnalyzerFinding>, String> {
    Ok(stdout
        .lines()
        .filter(|l| l.starts_with('{'))
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .filter_map(|line| {
            let message = &line["message"];
            let rule = message["code"]["code"].as_str().filter(|c| !c.is_empty())?;
            if line["reason"].as_str() != Some("compiler-message") {
                return None;
            }
            let level = message["level"].as_str()?;
            if !matches!(level, "warning" | "error") {
                return None;
            }
            let spans = message["spans"].as_array()?;
            let span = spans
                .iter()
                .find(|s| s["is_primary"].as_bool() == Some(true))
                .or_else(|| spans.first())?;
            let help: String = message["children"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|c| c["message"].as_str())
                .collect::<Vec<_>>()
                .join(" ");
            located(
                Draft {
                    rule_id: rule.to_owned(),
                    message: message["message"].as_str().unwrap_or(rule).to_owned(),
                    severity: if level == "error" {
                        Severity::High
                    } else {
                        Severity::Low
                    },
                    category: clippy_category(rule),
                    url: documentation_link(&help),
                },
                root,
                span["file_name"].as_str()?,
                Position {
                    line: num(&span["line_start"]),
                    column: num(&span["column_start"]),
                    end_line: num(&span["line_end"]),
                    end_column: num(&span["column_end"]),
                },
            )
        })
        .collect())
}

/// The first `https://rust-lang.github.io/...` link in Clippy's help text.
fn documentation_link(help: &str) -> Option<String> {
    let start = help.find("https://rust-lang.github.io/")?;
    let link: String = help[start..]
        .chars()
        .take_while(|c| !c.is_whitespace() && *c != '`')
        .collect();
    Some(link)
}

fn clippy_command(context: &RunContext<'_>) -> Invocation {
    // Build output goes to the temp directory, never into the repository's `target/`.
    let key = sha256_hex(&context.root.to_string_lossy());
    let target = std::env::temp_dir().join(format!("debuggatha-clippy-{}", &key[..12]));
    let mut args = strings(&[
        "--message-format=json",
        "--quiet",
        "--workspace",
        "--all-targets",
    ]);
    args.extend([
        "--target-dir".to_owned(),
        target.to_string_lossy().into_owned(),
    ]);
    Invocation {
        args,
        cleanup: None,
    }
}

pub static CLIPPY: Adapter = Adapter {
    id: "clippy",
    name: "Clippy",
    license: "MIT OR Apache-2.0",
    homepage: "https://doc.rust-lang.org/clippy",
    covers: "Rust",
    // Clippy compiles the crate: build scripts and procedural macros run, and missing dependencies
    // are downloaded.
    trust: Trust::RunsProjectCode,
    applies: |facts| facts.has_file_named(&["Cargo.toml"]),
    local_binaries: &[],
    binary_names: &["cargo"],
    prefix: &["clippy"],
    version_args: &["--version"],
    command: clippy_command,
    ok_exit_codes: &[0, 101],
    parse: clippy_parse,
    supersedes: &[],
    equivalent_to: &[],
    file_filter: None,
};

pub static ADAPTERS: &[&Adapter] = &[
    &GITLEAKS,
    &BIOME,
    &ESLINT,
    &RUFF,
    &KTLINT,
    &DETEKT,
    &PHPSTAN,
    &CLIPPY,
    &OSV_SCANNER,
];
