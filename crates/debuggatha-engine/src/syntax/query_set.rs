//! The queries of one adapter: one tree-sitter query file per language, each compiled the first
//! time its language is met (a repository holds a few languages, and every query costs
//! milliseconds).

use std::sync::OnceLock;

use super::{Language, Query};

pub struct QuerySet<const N: usize> {
    sources: [(Language, &'static str); N],
    compiled: [OnceLock<Result<Query, String>>; N],
}

impl<const N: usize> QuerySet<N> {
    #[must_use]
    pub const fn new(sources: [(Language, &'static str); N]) -> Self {
        Self {
            sources,
            compiled: [const { OnceLock::new() }; N],
        }
    }

    fn compiled(&self, index: usize) -> &Result<Query, String> {
        self.compiled[index].get_or_init(|| {
            let (language, source) = self.sources[index];
            Query::new(language, source).map_err(|e| e.to_string())
        })
    }

    /// The query of a language, if it has one and it compiles.
    #[must_use]
    pub fn query_for(&self, language: Language) -> Option<&Query> {
        let index = self.sources.iter().position(|(l, _)| *l == language)?;
        self.compiled(index).as_ref().ok()
    }

    /// The source text of each language's query.
    pub fn sources(&self) -> impl Iterator<Item = (Language, &'static str)> + '_ {
        self.sources.iter().copied()
    }

    /// Why a query does not compile, if any (a test asserts there is none).
    #[must_use]
    pub fn errors(&self) -> Vec<String> {
        self.sources
            .iter()
            .enumerate()
            .filter_map(|(i, (l, _))| {
                self.compiled(i)
                    .as_ref()
                    .err()
                    .map(|e| format!("{}: {e}", l.id()))
            })
            .collect()
    }
}
