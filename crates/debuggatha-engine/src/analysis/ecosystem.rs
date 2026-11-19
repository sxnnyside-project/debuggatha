//! Ecosystem descriptors: how a manifest names a module and how an import specifier of a language
//! reaches a file, a module of the repository or the outside. The descriptors are data
//! (`data/ecosystems.toml`); this module holds the few readers and resolution strategies they pick
//! from. A specifier that no repository file can answer is outside the repository, and one that
//! names the repository's own directories but no file is reported as not found.

mod names;
mod resolve;
mod xml;

use std::sync::OnceLock;

use serde::Deserialize;

pub use names::Dependency;

use super::structure::resolve::Resolution;
use super::structure::workspace::Workspace;
use crate::syntax::Language;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Descriptor {
    pub language: String,
    /// A file name, or `*.ext` for a manifest named after its project.
    pub manifest: String,
    /// How the module's name is read from the manifest.
    pub name: Option<String>,
    pub resolve: String,
    /// Source roots, relative to the repository root and to each manifest's directory.
    #[serde(default)]
    pub roots: Vec<String>,
    /// Extensions of the language's source files.
    pub extensions: Vec<String>,
    /// Directories whose children are the modules of a repository with one manifest (`""` is the
    /// root itself).
    #[serde(default)]
    pub module_roots: Vec<String>,
    /// Directories whose declarations the language keeps out of reach of other modules.
    #[serde(default)]
    pub internal_dirs: Vec<String>,
    /// How the manifest lists what it depends on.
    pub dependencies: Option<String>,
}

/// A manifest found in the repository.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestFile {
    pub ecosystem: String,
    pub path: String,
    pub dir: String,
    pub name: Option<String>,
    /// PSR-4 namespace prefixes and their directories, for Composer.
    pub namespaces: Vec<(String, String)>,
}

/// # Panics
/// If `data/ecosystems.toml` does not parse; a test keeps it valid.
#[must_use]
#[allow(clippy::expect_used)] // a table that does not parse is a build defect, caught by a test
pub fn descriptors() -> &'static [(String, Descriptor)] {
    static TABLE: OnceLock<Vec<(String, Descriptor)>> = OnceLock::new();
    TABLE.get_or_init(|| {
        let table: std::collections::BTreeMap<String, Descriptor> =
            toml::from_str(include_str!("../../data/ecosystems.toml"))
                .expect("data/ecosystems.toml is valid");
        table.into_iter().collect()
    })
}

pub struct Ecosystems<'w> {
    ws: &'w Workspace,
    manifests: Vec<ManifestFile>,
}

impl<'w> Ecosystems<'w> {
    #[must_use]
    pub fn new(ws: &'w Workspace) -> Self {
        let mut manifests = Vec::new();
        for (id, descriptor) in descriptors() {
            for path in ws.files_named(&descriptor.manifest) {
                if let Some(text) = ws.read(path) {
                    manifests.push(names::read(id, descriptor, path, text));
                }
            }
        }
        Self { ws, manifests }
    }

    #[must_use]
    pub fn manifests(&self) -> &[ManifestFile] {
        &self.manifests
    }

    /// What a manifest declares it depends on.
    #[must_use]
    pub fn dependencies(&self, manifest: &ManifestFile) -> Vec<Dependency> {
        let spec = descriptors()
            .iter()
            .find(|(id, _)| *id == manifest.ecosystem)
            .and_then(|(_, d)| d.dependencies.as_deref());
        match (spec, self.ws.read(&manifest.path)) {
            (Some(spec), Some(text)) => names::dependencies(spec, &manifest.path, text),
            _ => Vec::new(),
        }
    }

    /// The descriptor of a language, if one exists.
    #[must_use]
    pub fn of(language: Language) -> Option<(&'static str, &'static Descriptor)> {
        descriptors()
            .iter()
            .find(|(_, d)| d.language == language.id())
            .map(|(id, d)| (id.as_str(), d))
    }

    /// What `specifier`, written in `from` in `language`, names. `None` when no descriptor reads
    /// the language.
    #[must_use]
    pub fn resolve(&self, from: &str, language: Language, specifier: &str) -> Option<Resolution> {
        let (id, descriptor) = Self::of(language)?;
        Some(resolve::resolve(
            self.ws,
            &self.manifests,
            id,
            descriptor,
            from,
            specifier,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::descriptors;

    #[test]
    fn the_table_parses() {
        assert!(!descriptors().is_empty());
    }
}
