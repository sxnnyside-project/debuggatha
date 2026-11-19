//! `document-env-vars`: the environment variables the code reads and no file of the repository
//! mentions. [`env_docs`](super::env_docs) reads the files into the set of names they mention; this
//! module reads the code, in every language the engine parses, and reports a variable read and not
//! in that set.
//!
//! **Reads**: JavaScript and TypeScript through oxc (`process.env.X`, `import.meta.env.X`,
//! `Bun.env.X`, `Deno.env.get('X')`, a destructured `process.env`); every other language through a
//! tree-sitter query (`data/queries/env-*.scm`) that finds a call or index whose first argument is a
//! string literal, plus a table of the callees that read the environment. A name that is not a
//! literal, a write, test files, templates, styles and HTML are left alone, and the coverage report
//! says so.
//!
//! **Reported**: the first read of each undocumented variable in a file, at medium confidence (the
//! variable may be documented elsewhere, or provided by the platform). Variables that platforms and
//! tools own are on a short list ([`AMBIENT`], [`AMBIENT_PREFIXES`]).

use std::sync::OnceLock;

use oxc_allocator::Allocator;
use oxc_ast::ast::{
    AssignmentExpression, BindingPattern, CallExpression, Expression, PropertyKey, UnaryExpression,
    UnaryOperator, VariableDeclarator,
};
use oxc_ast_visit::{Visit, walk};
use oxc_parser::Parser;
use oxc_span::GetSpan;

use super::calls::one_per_line;
use super::env_docs::is_env_name;
use super::js::peel;
use super::js::{Lines, make_hit};
use crate::detect::coverage::GapReason;
use crate::detect::hit::Hit;
use crate::detect::input::DetectorInput;
use crate::detect::runner::Registry;
use crate::scan_scope::is_test_path;
use crate::syntax::oxc::source_type;
use crate::syntax::{FileSyntax, Language, Query, QueryMatch, script_regions};
use debuggatha_core::review_engine::{Confidence, Severity};

/// The rules this detector answers for.
pub const RULES: &[&str] = &["document-env-vars"];

/// Rules that need the repository's configuration and documentation files to conclude. Without
/// them the rule cannot run, and coverage says so.
pub const NEEDS_REPO_FILES: &[&str] = &["document-env-vars"];

/// What the reader leaves out, as coverage states it.
const LEFT_OUT: &str =
    "no environment reader for HTML or for the templates of single-file components";

/// Registers the detector.
pub fn register(registry: &mut Registry) {
    registry.register_group(RULES, detect);
    registry.mark_partial_as(RULES[0], GapReason::NoDetector, LEFT_OUT);
}

/// Variables that the operating system, a shell, a CI service or a build tool provide, so no
/// project documents them.
pub const AMBIENT: &[&str] = &[
    "PATH",
    "HOME",
    "USER",
    "USERNAME",
    "LOGNAME",
    "PWD",
    "OLDPWD",
    "SHELL",
    "TERM",
    "TMPDIR",
    "TEMP",
    "TMP",
    "TZ",
    "LANG",
    "LANGUAGE",
    "HOSTNAME",
    "EDITOR",
    "VISUAL",
    "PAGER",
    "DISPLAY",
    "NO_COLOR",
    "FORCE_COLOR",
    "CLICOLOR",
    "CLICOLOR_FORCE",
    "COLORTERM",
    "COLUMNS",
    "LINES",
    "CI",
    "NODE_ENV",
    "APPDATA",
    "LOCALAPPDATA",
    "USERPROFILE",
    "PROGRAMFILES",
    "SYSTEMROOT",
    "COMSPEC",
    "OUT_DIR",
    "TARGET",
    "PROFILE",
    "HOST",
    "OPT_LEVEL",
    "NUM_JOBS",
    "RUST_LOG",
    "RUST_BACKTRACE",
    "DEBUG",
    "NODE_OPTIONS",
    "NODE_PATH",
    "VITEST",
    "JEST_WORKER_ID",
    "HTTP_PROXY",
    "HTTPS_PROXY",
    "ALL_PROXY",
    "NO_PROXY",
    "http_proxy",
    "https_proxy",
    "all_proxy",
    "no_proxy",
    "SOURCE_DATE_EPOCH",
    "WSL_DISTRO_NAME",
    "TERM_PROGRAM",
    "DYNO",
    "VERCEL",
    "NETLIFY",
    "CF_PAGES",
    "RENDER",
    "GCP_BUILDPACKS",
    "DEPLOY_URL",
    "DEPLOY_PRIME_URL",
    // Netlify's build context and the SST platform's marker; Windows' executable extensions.
    "CONTEXT",
    "SST",
    "PATHEXT",
    // Set by Elixir releases for the scripts and the code they start.
    "RELEASE_ROOT",
    "RELEASE_NAME",
    "RELEASE_VSN",
    "RELEASE_NODE",
    "RELEASE_COOKIE",
    "RELEASE_TMP",
    "RELEASE_MODE",
    "RELEASE_DISTRIBUTION",
    "RELEASE_SYS_CONFIG",
    "RELEASE_VM_ARGS",
];

/// Variables Vite and its relatives define on `import.meta.env` themselves.
const BUNDLER_BUILT_INS: &[&str] = &["BASE_URL", "MODE", "DEV", "PROD", "SSR"];

/// Families of variables owned by a platform or tool.
pub const AMBIENT_PREFIXES: &[&str] = &[
    "LC_",
    "XDG_",
    "CARGO_",
    "RUSTC",
    "GITHUB_",
    "RUNNER_",
    "GITLAB_",
    "CI_",
    "npm_",
    "PYTHON",
    "OTEL_",
    "VERCEL_",
    "NETLIFY_",
    "CF_PAGES_",
    "RENDER_",
    "VSCODE_",
    "FLASK_",
    "PUPPETEER_",
];

fn is_ambient(name: &str) -> bool {
    AMBIENT.contains(&name) || AMBIENT_PREFIXES.iter().any(|p| name.starts_with(p))
}

/// One read of an environment variable in code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnvRead {
    pub name: String,
    pub start: usize,
    pub end: usize,
    /// The code handles the variable being absent: a default value, or a test for presence. The
    /// program starts without it, so leaving it undocumented costs less.
    pub fallback: bool,
}

/// The query of each language that has one, and the callees (or indexed objects) that read the
/// environment, with whitespace removed.
const READERS: &[(Language, &str, &[&str])] = &[
    (
        Language::Rust,
        include_str!("../../../data/queries/env-rust.scm"),
        &[
            "env::var",
            "std::env::var",
            "::std::env::var",
            "env::var_os",
            "std::env::var_os",
            "::std::env::var_os",
            "env",
            "option_env",
        ],
    ),
    (
        Language::Python,
        include_str!("../../../data/queries/env-python.scm"),
        &[
            "os.getenv",
            "getenv",
            "os.environ.get",
            "environ.get",
            "os.environ",
            "environ",
        ],
    ),
    (
        Language::Go,
        include_str!("../../../data/queries/env-go.scm"),
        &["os.Getenv", "os.LookupEnv"],
    ),
    (
        Language::Php,
        include_str!("../../../data/queries/env-php.scm"),
        &["getenv", "\\getenv", "env", "\\env", "$_ENV"],
    ),
    (
        Language::Java,
        include_str!("../../../data/queries/env-java.scm"),
        &["System.getenv"],
    ),
    (
        Language::Kotlin,
        include_str!("../../../data/queries/env-kotlin.scm"),
        &["System.getenv"],
    ),
    (
        Language::CSharp,
        include_str!("../../../data/queries/env-csharp.scm"),
        &[
            "Environment.GetEnvironmentVariable",
            "System.Environment.GetEnvironmentVariable",
        ],
    ),
    (
        Language::Ruby,
        include_str!("../../../data/queries/env-ruby.scm"),
        &["ENV", "ENV.fetch"],
    ),
    (
        Language::Swift,
        include_str!("../../../data/queries/env-swift.scm"),
        &["getenv", "ProcessInfo.processInfo.environment"],
    ),
    (
        Language::Dart,
        include_str!("../../../data/queries/env-dart.scm"),
        &[
            "Platform.environment",
            "String.fromEnvironment",
            "bool.fromEnvironment",
            "int.fromEnvironment",
        ],
    ),
    (
        Language::Elixir,
        include_str!("../../../data/queries/env-elixir.scm"),
        &["System.get_env", "System.fetch_env!", "System.fetch_env"],
    ),
    (
        Language::Cpp,
        include_str!("../../../data/queries/env-cpp.scm"),
        &["getenv", "std::getenv", "secure_getenv"],
    ),
    (
        Language::GdScript,
        include_str!("../../../data/queries/env-gdscript.scm"),
        &["OS.get_environment"],
    ),
];

/// Compiled on first use: a repository holds a few languages, and every query costs milliseconds.
fn compiled(index: usize) -> &'static Result<Query, String> {
    static QUERIES: [OnceLock<Result<Query, String>>; READERS.len()] =
        [const { OnceLock::new() }; READERS.len()];
    QUERIES[index].get_or_init(|| {
        let (language, source, _) = READERS[index];
        Query::new(language, source).map_err(|e| e.to_string())
    })
}

/// Why a query does not compile, if any (a test asserts there is none).
#[must_use]
pub fn compile_errors() -> Vec<String> {
    READERS
        .iter()
        .enumerate()
        .filter_map(|(i, (l, _, _))| {
            compiled(i)
                .as_ref()
                .err()
                .map(|e| format!("{}: {e}", l.id()))
        })
        .collect()
}

/// The languages whose code is read for environment variables.
#[must_use]
pub fn languages() -> Vec<Language> {
    let mut all = vec![
        Language::JavaScript,
        Language::Jsx,
        Language::TypeScript,
        Language::Tsx,
    ];
    all.extend(READERS.iter().map(|(l, ..)| *l));
    all
}

/// The name a string literal spells, if it spells one: quotes off, no escape, no interpolation, and
/// shaped like a variable name.
fn literal_name(text: &str) -> Option<String> {
    let text = text.trim();
    let text = text.strip_prefix(['r', 'R', 'u', 'U']).unwrap_or(text);
    let quote = text
        .chars()
        .next()
        .filter(|c| matches!(c, '"' | '\'' | '`'))?;
    let inner = text.strip_prefix(quote)?.strip_suffix(quote)?;
    (is_env_name(inner)).then(|| inner.to_owned())
}

fn squash(text: &str) -> String {
    text.chars().filter(|c| !c.is_whitespace()).collect()
}

fn tree_sitter_reads(syntax: &FileSyntax, content: &str) -> Vec<EnvRead> {
    let Some(index) = READERS.iter().position(|(l, ..)| *l == syntax.language) else {
        return Vec::new();
    };
    let (_, _, callees) = READERS[index];
    let Ok(query) = compiled(index) else {
        return Vec::new();
    };
    let Ok(matches) = query.run(syntax, content) else {
        return Vec::new();
    };
    let text = |span: crate::syntax::Span| content.get(span.start as usize..span.end as usize);
    let capture =
        |m: &QueryMatch, name: &str| m.captures.iter().find(|c| c.name == name).map(|c| c.span);
    // A write names the same literal a read pattern also matched.
    let writes: Vec<_> = matches
        .iter()
        .filter_map(|m| capture(m, "w_value"))
        .collect();
    // The literals that have a default beside them, or a handler for their absence after them.
    let defaulted: Vec<_> = matches
        .iter()
        .filter(|m| {
            capture(m, "default").is_some()
                || capture(m, "fallback")
                    .and_then(text)
                    .is_some_and(|f| FALLBACK_METHODS.contains(&f))
        })
        .filter_map(|m| capture(m, "value"))
        .collect();
    let mut reads = Vec::new();
    for m in &matches {
        let (Some(callee), Some(value)) = (capture(m, "callee"), capture(m, "value")) else {
            continue;
        };
        if writes.contains(&value) {
            continue;
        }
        let Some(callee_text) = text(callee) else {
            continue;
        };
        let mut callee_name = squash(callee_text);
        // Java and Ruby split the receiver from the method.
        if let Some(method) = capture(m, "method").and_then(text) {
            callee_name = format!("{callee_name}.{method}");
        }
        if !callees.contains(&callee_name.as_str()) {
            continue;
        }
        if let Some(name) = text(value).and_then(literal_name) {
            reads.push(EnvRead {
                name,
                start: value.start as usize,
                end: value.end as usize,
                fallback: defaulted.contains(&value)
                    || matches!(callee_name.as_str(), "os.LookupEnv" | "option_env"),
            });
        }
    }
    reads.sort_by_key(|r| r.start);
    reads.dedup_by_key(|r| r.start);
    reads
}

/// Methods of a Rust `Result` or `Option` that handle the variable being absent.
const FALLBACK_METHODS: &[&str] = &[
    "unwrap_or",
    "unwrap_or_else",
    "unwrap_or_default",
    "ok",
    "is_ok",
    "is_err",
    "map_or",
    "map_or_else",
];

/// `process.env`, `import.meta.env`, `Bun.env`: the object that holds the variables.
fn is_env_object(e: &Expression<'_>) -> bool {
    let Expression::StaticMemberExpression(m) = peel(e) else {
        return false;
    };
    if m.property.name != "env" {
        return false;
    }
    match peel(&m.object) {
        Expression::Identifier(id) => matches!(id.name.as_str(), "process" | "Bun"),
        Expression::ImportMeta(_) => true,
        _ => false,
    }
}

#[derive(Default)]
struct JsReads {
    reads: Vec<EnvRead>,
    /// Spans that have a default: the left side of `||` and `??`.
    defaults: Vec<(usize, usize)>,
    /// Spans that are written or deleted, not read.
    writes: Vec<(usize, usize)>,
}

impl JsReads {
    fn push(&mut self, name: &str, span: oxc_span::Span, fallback: bool) {
        if is_env_name(name) {
            self.reads.push(EnvRead {
                name: name.to_owned(),
                start: span.start as usize,
                end: span.end as usize,
                fallback,
            });
        }
    }
}

impl<'a> Visit<'a> for JsReads {
    fn visit_static_member_expression(&mut self, it: &oxc_ast::ast::StaticMemberExpression<'a>) {
        if is_env_object(&it.object) {
            self.push(&it.property.name, it.span, false);
        }
        walk::walk_static_member_expression(self, it);
    }

    fn visit_computed_member_expression(
        &mut self,
        it: &oxc_ast::ast::ComputedMemberExpression<'a>,
    ) {
        if is_env_object(&it.object)
            && let Expression::StringLiteral(s) = peel(&it.expression)
        {
            self.push(&s.value.display().to_string(), it.span, false);
        }
        walk::walk_computed_member_expression(self, it);
    }

    fn visit_call_expression(&mut self, it: &CallExpression<'a>) {
        // Deno.env.get("X")
        if let Expression::StaticMemberExpression(m) = peel(&it.callee)
            && m.property.name == "get"
            && let Expression::StaticMemberExpression(env) = peel(&m.object)
            && env.property.name == "env"
            && matches!(peel(&env.object), Expression::Identifier(id) if id.name == "Deno")
            && let Some(Expression::StringLiteral(s)) =
                it.arguments.first().and_then(|a| a.as_expression())
        {
            self.push(&s.value.display().to_string(), it.span, false);
        }
        walk::walk_call_expression(self, it);
    }

    fn visit_variable_declarator(&mut self, it: &VariableDeclarator<'a>) {
        // const { A, B: b } = process.env
        if let (Some(init), BindingPattern::ObjectPattern(pattern)) = (&it.init, &it.id)
            && is_env_object(init)
        {
            for p in &pattern.properties {
                if let PropertyKey::StaticIdentifier(id) = &p.key {
                    // `const { PORT = 3000 } = process.env`
                    let fallback = matches!(p.value, BindingPattern::AssignmentPattern(_));
                    self.push(&id.name, p.span, fallback);
                }
            }
        }
        walk::walk_variable_declarator(self, it);
    }

    fn visit_assignment_expression(&mut self, it: &AssignmentExpression<'a>) {
        let span = it.left.span();
        self.writes.push((span.start as usize, span.end as usize));
        walk::walk_assignment_expression(self, it);
    }

    fn visit_logical_expression(&mut self, it: &oxc_ast::ast::LogicalExpression<'a>) {
        if matches!(
            it.operator,
            oxc_ast::ast::LogicalOperator::Or | oxc_ast::ast::LogicalOperator::Coalesce
        ) {
            let span = peel(&it.left).span();
            self.defaults.push((span.start as usize, span.end as usize));
        }
        walk::walk_logical_expression(self, it);
    }

    fn visit_unary_expression(&mut self, it: &UnaryExpression<'a>) {
        if it.operator == UnaryOperator::Delete {
            let span = it.argument.span();
            self.writes.push((span.start as usize, span.end as usize));
        }
        walk::walk_unary_expression(self, it);
    }
}

fn js_reads(text: &str, language: Language) -> Vec<EnvRead> {
    let allocator = Allocator::default();
    // A frontmatter's top-level `return` is reported and recovered from; the reads after it stay.
    let parsed = Parser::new(&allocator, text, source_type(language, false)).parse();
    let mut reads = JsReads::default();
    reads.visit_program(&parsed.program);
    let JsReads {
        reads,
        defaults,
        writes,
    } = reads;
    reads
        .into_iter()
        .filter(|r| !writes.iter().any(|&(a, b)| a <= r.start && r.end <= b))
        .map(|mut r| {
            r.fallback |= defaults.contains(&(r.start, r.end));
            r
        })
        .filter(|r| {
            // `import.meta.env.MODE` is the bundler's own, whatever the project defines.
            let code = text.get(r.start..r.end).unwrap_or("");
            !(code.contains("import.meta.env") && BUNDLER_BUILT_INS.contains(&r.name.as_str()))
        })
        .collect()
}

/// Every read of an environment variable by name in the file, in source order.
#[must_use]
pub fn reads_of(input: &DetectorInput<'_>) -> Vec<EnvRead> {
    let Some(syntax) = input.syntax else {
        return Vec::new();
    };
    match syntax.language {
        Language::JavaScript | Language::Jsx | Language::TypeScript | Language::Tsx => {
            js_reads(input.content, syntax.language)
        }
        // The scripts of a component, moved back to offsets in the file.
        Language::Vue | Language::Svelte | Language::Astro => {
            script_regions(input.content, syntax.language)
                .into_iter()
                .flat_map(|region| {
                    let code = &input.content[region.start..region.end];
                    js_reads(code, region.language)
                        .into_iter()
                        .map(move |mut r| {
                            r.start += region.start;
                            r.end += region.start;
                            r
                        })
                })
                .collect()
        }
        _ => tree_sitter_reads(syntax, input.content),
    }
}

/// Runs the detector over one file.
#[must_use]
pub fn detect(input: &DetectorInput<'_>) -> Vec<(&'static str, Hit)> {
    // Without the repository's files the rule cannot conclude: it reports nothing, and coverage
    // lists it as a gap. It never reads that as "nothing is documented".
    let Some(docs) = input.env_docs else {
        return Vec::new();
    };
    if is_test_path(input.file) {
        return Vec::new();
    }
    let lines = Lines::new(input.content);
    let mut seen = std::collections::BTreeSet::new();
    let mut hits = Vec::new();
    for read in reads_of(input) {
        if is_ambient(&read.name) || docs.documents(&read.name) || !seen.insert(read.name.clone()) {
            continue;
        }
        // Optional configuration costs less when it is missing: low confidence, low severity.
        let confidence = if read.fallback {
            Confidence::Low
        } else {
            Confidence::Medium
        };
        if let Some(mut hit) = make_hit(input, &lines, read.start, read.end, confidence, None) {
            if read.fallback {
                hit.severity = Some(Severity::Low);
            }
            hits.push((RULES[0], hit, read.start));
        }
    }
    one_per_line(hits)
}
