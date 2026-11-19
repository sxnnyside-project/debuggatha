//! What a repository says about its environment variables, read from files that are not source:
//! example dotenv files, the README, a Dockerfile, compose, CI workflows, `Makefile` or `Justfile`,
//! package manager manifests and platform files (`wrangler.toml`, `vercel.json`, `fly.toml`,
//! `Procfile`). It does not depend on the language of the code.
//!
//! A name is mentioned when a file assigns it (`NAME=value`, `NAME: value`, `NAME ?= ..`), expands
//! it (`$NAME`, `${NAME:-default}`), lists it alone (`- NAME`), declares it (`ENV`, `ARG`) or, in
//! prose, writes it as a bare upper-case word in a Markdown code span or fence. Names only: it never
//! keeps what a variable is set to.

// `doc_kind` lower-cases the path before it compares suffixes, so the comparisons below are
// case-insensitive already.
#![allow(clippy::case_sensitive_file_extension_comparisons)]

use std::collections::BTreeSet;

use crate::detect::input::RepoFile;

/// The kinds of file that document configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocKind {
    /// `.env.example`, `.env`, `.env.production`, `example.env`, `.envrc`.
    Dotenv,
    /// `README`, `README.md`, `docs/*.md`.
    Markdown,
    /// `Dockerfile`, `Dockerfile.dev`, `app.dockerfile`, `Containerfile`.
    Dockerfile,
    /// Compose, CI, build and manifest files, read for names they assign or expand.
    Config,
    /// A build script that defines compile-time variables: `println!("cargo:rustc-env=NAME=..")`.
    RustSource,
}

/// Base names (lower case) of configuration files read as [`DocKind::Config`].
const CONFIG_NAMES: &[&str] = &[
    "makefile",
    "gnumakefile",
    "justfile",
    "taskfile.yml",
    "taskfile.yaml",
    "package.json",
    "composer.json",
    "pyproject.toml",
    "pipfile",
    "tox.ini",
    "procfile",
    "app.json",
    "wrangler.toml",
    "wrangler.json",
    "wrangler.jsonc",
    "vercel.json",
    "netlify.toml",
    "fly.toml",
    "render.yaml",
    "render.yml",
    ".gitlab-ci.yml",
    "azure-pipelines.yml",
    "bitbucket-pipelines.yml",
    "cloudbuild.yaml",
    "cloudbuild.yml",
    "mix.exs",
    "pubspec.yaml",
];

/// What follows `env.` or `.env.` in the name of a dotenv file (`.env.local`, `env.example`).
const DOTENV_SUFFIXES: &[&str] = &[
    "example",
    "sample",
    "template",
    "dist",
    "defaults",
    "local",
    "dev",
    "development",
    "prod",
    "production",
    "test",
    "staging",
    "ci",
];

fn is_dotenv(base: &str) -> bool {
    if base == ".env" || base == ".envrc" || base.ends_with(".env") {
        return true;
    }
    // `.env.example`, `.env.production.local`, `env.example`, `app.env.sample`
    let after = base
        .strip_prefix(".env.")
        .or_else(|| base.strip_prefix("env."))
        .or_else(|| base.split_once(".env.").map(|(_, rest)| rest));
    after.is_some_and(|rest| rest.split('.').all(|part| DOTENV_SUFFIXES.contains(&part)))
}

/// How a path is read, if it is read at all.
#[must_use]
pub fn doc_kind(path: &str) -> Option<DocKind> {
    let normalized = path.replace('\\', "/");
    let lower = normalized.to_ascii_lowercase();
    let base = lower.rsplit('/').next().unwrap_or(&lower);
    if is_dotenv(base) {
        return Some(DocKind::Dotenv);
    }
    // Cargo's default build script. A `build = "build/main.rs"` elsewhere is not read: the file
    // listing skips `build/` directories, which hold output.
    if base == "build.rs" {
        return Some(DocKind::RustSource);
    }
    if base.starts_with("dockerfile")
        || base.ends_with(".dockerfile")
        || base.starts_with("containerfile")
    {
        return Some(DocKind::Dockerfile);
    }
    if base.starts_with("readme")
        || base.ends_with(".md")
        || base.ends_with(".mdx")
        || base.ends_with(".markdown")
        || base.ends_with(".rst")
        || base.ends_with(".adoc")
    {
        return Some(DocKind::Markdown);
    }
    let is_yaml = base.ends_with(".yml") || base.ends_with(".yaml");
    let is_compose = is_yaml && (base.starts_with("docker-compose") || base.starts_with("compose"));
    let is_workflow = is_yaml
        && (lower.contains(".github/workflows/")
            || lower.contains(".circleci/")
            || lower.contains(".buildkite/")
            || lower.contains(".woodpecker/"));
    let is_cargo_config = lower.ends_with(".cargo/config.toml") || lower.ends_with(".cargo/config");
    if is_compose
        || is_workflow
        || is_cargo_config
        || base.ends_with(".mk")
        || CONFIG_NAMES.contains(&base)
    {
        return Some(DocKind::Config);
    }
    None
}

/// Is `name` shaped like the name of an environment variable?
#[must_use]
pub fn is_env_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// An upper-case word such as `DATABASE_URL` or `PORT`.
fn is_upper_word(word: &str) -> bool {
    word.len() >= 2
        && is_env_name(word)
        && word.chars().any(|c| c.is_ascii_uppercase())
        && !word.chars().any(|c| c.is_ascii_lowercase())
}

/// The names the repository's files mention.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EnvDocs {
    names: BTreeSet<String>,
    /// How many of the files given were read (the rest are not documentation).
    files_read: usize,
}

impl EnvDocs {
    /// Reads every file that is documentation or configuration; the others are ignored.
    #[must_use]
    pub fn build(files: &[RepoFile]) -> Self {
        let mut docs = Self::default();
        for file in files {
            let Some(kind) = doc_kind(&file.path) else {
                continue;
            };
            if kind == DocKind::RustSource {
                // Only a file that defines variables counts as a file read.
                docs.files_read += usize::from(rustc_env(&file.content, &mut docs.names));
                continue;
            }
            docs.files_read += 1;
            match kind {
                DocKind::Dotenv => dotenv(&file.content, &mut docs.names),
                DocKind::Markdown => markdown(&file.content, &mut docs.names),
                DocKind::Dockerfile => dockerfile(&file.content, &mut docs.names),
                DocKind::Config => config(&file.content, &mut docs.names),
                DocKind::RustSource => {}
            }
        }
        docs
    }

    /// Does any file mention the variable?
    #[must_use]
    pub fn documents(&self, name: &str) -> bool {
        self.names.contains(name)
    }

    /// The names mentioned, sorted.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.names.iter().map(String::as_str)
    }

    /// How many files were read as documentation or configuration.
    #[must_use]
    pub const fn files_read(&self) -> usize {
        self.files_read
    }
}

/// `NAME=value`, `export NAME=value`, and the same behind `#` (an example often comments out the
/// optional ones).
fn dotenv(text: &str, out: &mut BTreeSet<String>) {
    for line in text.lines() {
        let mut rest = line.trim_start().trim_start_matches('#').trim_start();
        if let Some(after) = rest.strip_prefix("export ") {
            rest = after.trim_start();
        }
        let end = rest
            .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
            .unwrap_or(rest.len());
        let (name, tail) = rest.split_at(end);
        if is_env_name(name) && tail.trim_start().starts_with('=') {
            out.insert(name.to_owned());
        }
    }
}

/// Names written in code: fenced blocks, and inline code spans.
fn markdown(text: &str, out: &mut BTreeSet<String>) {
    let mut fence: Option<&str> = None;
    for line in text.lines() {
        let trimmed = line.trim_start();
        let marker = ["```", "~~~"].into_iter().find(|m| trimmed.starts_with(m));
        match (fence, marker) {
            (None, Some(m)) => fence = Some(m),
            (Some(open), Some(m)) if open == m => fence = None,
            (Some(_), _) => scan(line, true, out),
            (None, None) => {
                // Inline code: the text between runs of backticks (` or ``, as reStructuredText
                // writes it). A run toggles; its length does not matter.
                let (mut in_code, mut in_run, mut code) = (false, false, String::new());
                for ch in line.chars() {
                    if ch == '`' {
                        if !in_run {
                            in_code = !in_code;
                            if !in_code {
                                scan(&code, true, out);
                                code.clear();
                            }
                        }
                        in_run = true;
                    } else {
                        in_run = false;
                        if in_code {
                            code.push(ch);
                        }
                    }
                }
            }
        }
    }
}

/// `ENV A=1 B=2`, `ENV A value`, `ARG A`, `ARG A=default`, plus any `$A` expansion.
fn dockerfile(text: &str, out: &mut BTreeSet<String>) {
    let mut logical = String::new();
    for line in text.lines() {
        let line = line.trim_end();
        if let Some(stripped) = line.strip_suffix('\\') {
            logical.push_str(stripped);
            logical.push(' ');
            continue;
        }
        logical.push_str(line);
        instruction(&logical, out);
        logical.clear();
    }
    if !logical.is_empty() {
        instruction(&logical, out);
    }
    for line in text.lines() {
        if !line.trim_start().starts_with('#') {
            expansions(line, out);
        }
    }
}

fn instruction(line: &str, out: &mut BTreeSet<String>) {
    let line = line.trim_start();
    let Some((keyword, rest)) = line.split_once(char::is_whitespace) else {
        return;
    };
    if !(keyword.eq_ignore_ascii_case("ENV") || keyword.eq_ignore_ascii_case("ARG")) {
        return;
    }
    let rest = rest.trim();
    // `ENV A=1 B="two words"`: every word with an `=` names a variable.
    let mut in_quote: Option<char> = None;
    let mut word = String::new();
    let mut words = Vec::new();
    for c in rest.chars() {
        match (c, in_quote) {
            ('"' | '\'', None) => {
                in_quote = Some(c);
                word.push(c);
            }
            (q, Some(open)) if q == open => {
                in_quote = None;
                word.push(c);
            }
            (c, None) if c.is_whitespace() => {
                if !word.is_empty() {
                    words.push(std::mem::take(&mut word));
                }
            }
            (c, _) => word.push(c),
        }
    }
    if !word.is_empty() {
        words.push(word);
    }
    for w in &words {
        if let Some((name, _)) = w.split_once('=')
            && is_env_name(name)
        {
            out.insert(name.to_owned());
        }
    }
    // `ENV A value` and `ARG A`: the first word is the name.
    if let Some(first) = words.first()
        && is_env_name(first)
    {
        out.insert(first.clone());
    }
}

/// Compose, CI, build and manifest files: names assigned, expanded or listed.
fn config(text: &str, out: &mut BTreeSet<String>) {
    for line in text.lines() {
        scan(line, false, out);
        // A YAML or compose list item that is a name and nothing else: `- API_KEY`.
        if let Some(item) = line.trim_start().strip_prefix("- ") {
            let item = item.split('#').next().unwrap_or("").trim();
            if is_upper_word(item) {
                out.insert(item.to_owned());
            }
        }
    }
}

/// `cargo:rustc-env=NAME=value` (or `cargo::`), which a build script prints: the repository defines
/// the variable itself, so `env!("NAME")` is not configuration someone has to supply. Returns
/// whether the file defined any.
fn rustc_env(text: &str, out: &mut BTreeSet<String>) -> bool {
    let mut found = false;
    for part in text.split("rustc-env=").skip(1) {
        let end = part
            .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
            .unwrap_or(part.len());
        if is_env_name(&part[..end]) && part[end..].starts_with('=') {
            out.insert(part[..end].to_owned());
            found = true;
        }
    }
    found
}

/// `$NAME`, `${NAME}`, `${NAME:-x}`.
fn expansions(text: &str, out: &mut BTreeSet<String>) {
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'$' {
            let start = if bytes.get(i + 1) == Some(&b'{') {
                i + 2
            } else {
                i + 1
            };
            let end = text[start..]
                .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
                .map_or(text.len(), |n| start + n);
            if is_env_name(&text[start..end]) {
                out.insert(text[start..end].to_owned());
            }
            i = end.max(i + 1);
        } else {
            i += 1;
        }
    }
}

/// The mention forms of one line (see the module documentation). `bare` also accepts an upper-case
/// word on its own, which is only safe inside code.
fn scan(line: &str, bare: bool, out: &mut BTreeSet<String>) {
    expansions(line, out);
    let mut start = None;
    let chars: Vec<(usize, char)> = line.char_indices().collect();
    for (k, &(i, c)) in chars.iter().enumerate() {
        let in_word = c.is_ascii_alphanumeric() || c == '_';
        match (start, in_word) {
            (None, true) => start = Some(i),
            (Some(s), false) => {
                word(line, s, i, bare, out);
                start = None;
            }
            _ => {}
        }
        if k + 1 == chars.len()
            && in_word
            && let Some(s) = start
        {
            word(line, s, line.len(), bare, out);
        }
    }
}

fn word(line: &str, start: usize, end: usize, bare: bool, out: &mut BTreeSet<String>) {
    let name = &line[start..end];
    if !is_env_name(name) {
        return;
    }
    let after = line[end..].trim_start_matches([' ', '\t', '"', '\'']);
    let assigns = after.starts_with(':') && {
        // `NAME: value`, not `a:b` inside a URL; a key is followed by a space or the end.
        let rest = &after[1..];
        rest.is_empty() || rest.starts_with([' ', '\t'])
    } || after.starts_with('=') && !after.starts_with("==")
        || after.starts_with("?=")
        || after.starts_with(":=")
        || after.starts_with("+=");
    // A key or assignment names a variable only in upper case (`run:` is a recipe, `env = [` a key).
    if (assigns || bare) && is_upper_word(name) {
        out.insert(name.to_owned());
    }
}
