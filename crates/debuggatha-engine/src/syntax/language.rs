//! Which languages the syntax layer knows, and which backend parses each (ADR-0006, decision 4).

use std::path::Path;

/// A language the engine can parse.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Language {
    JavaScript,
    Jsx,
    TypeScript,
    Tsx,
    Rust,
    Python,
    Go,
    Php,
    Java,
    CSharp,
    Ruby,
    Kotlin,
    Swift,
    Dart,
    Elixir,
    Cpp,
    GdScript,
    Html,
    Vue,
    Svelte,
    Astro,
}

/// Who parses a language: oxc for the JavaScript/TypeScript family, tree-sitter for the rest.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backend {
    Oxc,
    TreeSitter,
    /// A single-file component: its script blocks go to oxc (`sfc.rs`).
    Sfc,
}

impl Language {
    pub const ALL: [Self; 21] = [
        Self::JavaScript,
        Self::Jsx,
        Self::TypeScript,
        Self::Tsx,
        Self::Rust,
        Self::Python,
        Self::Go,
        Self::Php,
        Self::Java,
        Self::CSharp,
        Self::Ruby,
        Self::Kotlin,
        Self::Swift,
        Self::Dart,
        Self::Elixir,
        Self::Cpp,
        Self::GdScript,
        Self::Html,
        Self::Vue,
        Self::Svelte,
        Self::Astro,
    ];

    /// The id used in coverage reports (`Coverage.languages[].language`).
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::JavaScript => "javascript",
            Self::Jsx => "jsx",
            Self::TypeScript => "typescript",
            Self::Tsx => "tsx",
            Self::Rust => "rust",
            Self::Python => "python",
            Self::Go => "go",
            Self::Php => "php",
            Self::Java => "java",
            Self::CSharp => "csharp",
            Self::Ruby => "ruby",
            Self::Kotlin => "kotlin",
            Self::Swift => "swift",
            Self::Dart => "dart",
            Self::Elixir => "elixir",
            Self::Cpp => "cpp",
            Self::GdScript => "gdscript",
            Self::Html => "html",
            Self::Vue => "vue",
            Self::Svelte => "svelte",
            Self::Astro => "astro",
        }
    }

    #[must_use]
    pub const fn backend(self) -> Backend {
        match self {
            Self::JavaScript | Self::Jsx | Self::TypeScript | Self::Tsx => Backend::Oxc,
            Self::Vue | Self::Svelte | Self::Astro => Backend::Sfc,
            _ => Backend::TreeSitter,
        }
    }

    /// Supported with a stated limit; the coverage report says `parsed-limited`. C++: the grammar
    /// does not run the preprocessor, so macro-heavy code parses with errors (ADR-0006 spike A).
    /// Vue, Svelte and Astro: the scripts are parsed, the templates and styles are not.
    #[must_use]
    pub const fn limited(self) -> bool {
        matches!(self, Self::Cpp | Self::Vue | Self::Svelte | Self::Astro)
    }

    /// From the file name alone. `.js` is read as JSX because React code commonly keeps JSX in
    /// `.js`; a `.ts` file is never JSX (`<T>x` is a type assertion there). `.h` is not mapped:
    /// it is C as often as C++, and C has no grammar here.
    #[must_use]
    pub fn from_path(path: &Path) -> Option<Self> {
        let ext = path.extension()?.to_str()?;
        Some(match ext {
            "ts" | "mts" | "cts" => Self::TypeScript,
            "tsx" => Self::Tsx,
            "js" | "jsx" => Self::Jsx,
            "mjs" | "cjs" => Self::JavaScript,
            "rs" => Self::Rust,
            "py" | "pyi" => Self::Python,
            "go" => Self::Go,
            "php" => Self::Php,
            "java" => Self::Java,
            "cs" => Self::CSharp,
            "rb" => Self::Ruby,
            "kt" | "kts" => Self::Kotlin,
            "swift" => Self::Swift,
            "dart" => Self::Dart,
            "ex" | "exs" => Self::Elixir,
            "cpp" | "cc" | "cxx" | "hpp" | "hh" | "hxx" => Self::Cpp,
            "gd" => Self::GdScript,
            "html" | "htm" => Self::Html,
            "vue" => Self::Vue,
            "svelte" => Self::Svelte,
            "astro" => Self::Astro,
            _ => return None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{Backend, Language};
    use std::path::Path;

    #[test]
    fn extensions_map_to_languages() {
        let l = |p: &str| Language::from_path(Path::new(p));
        assert_eq!(l("a/b.ts"), Some(Language::TypeScript));
        assert_eq!(l("a.mts"), Some(Language::TypeScript));
        assert_eq!(l("a.tsx"), Some(Language::Tsx));
        assert_eq!(l("a.js"), Some(Language::Jsx));
        assert_eq!(l("a.mjs"), Some(Language::JavaScript));
        assert_eq!(l("a.rs"), Some(Language::Rust));
        assert_eq!(l("a.exs"), Some(Language::Elixir));
        assert_eq!(l("a.h"), None);
        assert_eq!(l("Makefile"), None);
    }

    #[test]
    fn ids_are_unique_and_every_language_has_a_backend() {
        let mut ids: Vec<_> = Language::ALL.iter().map(|l| l.id()).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), Language::ALL.len());
        let oxc = Language::ALL
            .iter()
            .filter(|l| l.backend() == Backend::Oxc)
            .count();
        assert_eq!(oxc, 4);
        assert_eq!(
            Language::ALL
                .iter()
                .filter(|l| l.backend() == Backend::Sfc)
                .count(),
            3
        );
    }
}
