use std::collections::BTreeSet;
use std::path::Path;

use super::env_docs::EnvDocs;
use super::modules::ModuleIndex;
use crate::syntax::{FileSyntax, SyntaxFacts, mask_source, parse};

/// One file handed to the engine. A detector always reads the **whole** file: for a diff review
/// `changed_lines` names the added lines and only hits on them are kept. (The TypeScript engine
/// handed detectors the added lines alone and parsed fragments; see `docs/PARITY.md`.)
#[derive(Debug, Clone)]
pub struct SourceUnit {
    pub file: String,
    pub content: String,
    /// 1-based lines of the file a diff added. `None` reviews the whole file.
    pub changed_lines: Option<BTreeSet<u32>>,
}

/// A file of the repository that is not reviewed as code but read for what it says about the
/// project: example dotenv files, the README, a Dockerfile, a compose file, a CI workflow, a
/// manifest of a package manager. Which of them count is decided by `env_docs::doc_kind`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoFile {
    /// Path relative to the repository root, with `/` separators.
    pub path: String,
    pub content: String,
}

/// What a detector sees of one file.
pub struct DetectorInput<'a> {
    pub file: &'a str,
    pub content: &'a str,
    /// `content` with comments and string contents blanked, same layout: what is left is code.
    /// Equal to `content` for a language with no parser.
    pub masked: String,
    /// The parse, when the engine has a parser for the file.
    pub syntax: Option<&'a FileSyntax>,
    /// What the other files of the review export, when a rule of the policy reads it
    /// (`modules::NEEDS_INDEX`). `None` also in a unit test that builds a single input.
    pub modules: Option<&'a ModuleIndex>,
    /// What the repository's configuration and documentation files say about environment
    /// variables, when the caller handed them over and a rule of the policy reads them
    /// (`env::NEEDS_REPO_FILES`). `None` means the review did not have them, which is not the same
    /// as "they document nothing".
    pub env_docs: Option<&'a EnvDocs>,
}

impl<'a> DetectorInput<'a> {
    #[must_use]
    pub fn new(file: &'a str, content: &'a str, syntax: Option<&'a FileSyntax>) -> Self {
        let masked = match syntax.and_then(|s| s.facts.as_ref()) {
            Some(facts) => mask_source(content, facts),
            None => content.to_owned(),
        };
        Self {
            file,
            content,
            masked,
            syntax,
            modules: None,
            env_docs: None,
        }
    }

    #[must_use]
    pub const fn with_modules(mut self, modules: Option<&'a ModuleIndex>) -> Self {
        self.modules = modules;
        self
    }

    #[must_use]
    pub const fn with_env_docs(mut self, env_docs: Option<&'a EnvDocs>) -> Self {
        self.env_docs = env_docs;
        self
    }

    #[must_use]
    pub fn facts(&self) -> Option<&SyntaxFacts> {
        self.syntax.and_then(|s| s.facts.as_ref())
    }
}

/// Parse a unit once for all rules.
pub(crate) fn parse_unit(unit: &SourceUnit) -> Option<FileSyntax> {
    parse(Path::new(&unit.file), &unit.content)
}
