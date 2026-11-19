//! `String.prototype.localeCompare` as the TypeScript engine uses it.
//!
//! Rules and packs are ordered with the default (root) ICU collation, which is not byte order:
//! `a` sorts before `B`, punctuation before digits, `é` next to `e`. ICU4X implements the same
//! collation as the ICU that Node and Bun use.

use std::cmp::Ordering;
use std::sync::OnceLock;

use icu_collator::options::CollatorOptions;
use icu_collator::{Collator, CollatorBorrowed};

fn collator() -> &'static CollatorBorrowed<'static> {
    static COLLATOR: OnceLock<CollatorBorrowed<'static>> = OnceLock::new();
    COLLATOR.get_or_init(|| {
        // Root locale, default options: tertiary strength, punctuation not ignored.
        Collator::try_new(
            icu_locale_core::Locale::UNKNOWN.into(),
            CollatorOptions::default(),
        )
        .unwrap_or_else(|_| unreachable!("compiled collation data for the root locale"))
    })
}

#[must_use]
pub fn locale_compare(a: &str, b: &str) -> Ordering {
    collator().compare(a, b)
}

#[cfg(test)]
mod tests {
    use super::locale_compare;

    #[test]
    fn sorts_like_node() {
        // Output of `[...xs].sort((a, b) => a.localeCompare(b))` in Node and in Bun.
        let expected = [
            " ",
            "_",
            "-",
            "10",
            "9",
            "a",
            "A",
            "a_b",
            "a-1",
            "a-b",
            "a:b",
            "a.b",
            "a1",
            "ab",
            "b",
            "B",
            "criteria:claude-md:10",
            "criteria:claude-md:3",
            "e",
            "é",
            "É",
            "f",
            "no-eval",
            "no-explicit-any",
            "ss",
            "ß",
            "x y",
            "xy",
            "z",
            "Z",
        ];
        let mut shuffled: Vec<&str> = expected.iter().rev().copied().collect();
        shuffled.sort_by(|a, b| locale_compare(a, b));
        assert_eq!(shuffled, expected);
    }
}
