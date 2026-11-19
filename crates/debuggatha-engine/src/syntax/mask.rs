//! Blanks comments and string contents so a rule reads only code.

use super::SyntaxFacts;

/// The source with comment text and string contents replaced by spaces, one space per UTF-16 unit
/// and newlines kept, so a match's line and column in the result are its line and column in the
/// source. What is hidden is decided by the backend (`SyntaxFacts::hidden`): quotes stay, comment
/// delimiters go with the comment.
#[must_use]
pub fn mask_source(source: &str, facts: &SyntaxFacts) -> String {
    let mut hidden: Vec<(usize, usize)> = facts
        .hidden
        .iter()
        .map(|s| (s.start as usize, s.end as usize))
        .collect();
    hidden.sort_unstable();
    // Merge overlaps, so one cursor over the sorted ranges is enough.
    let mut merged: Vec<(usize, usize)> = Vec::with_capacity(hidden.len());
    for (start, end) in hidden {
        match merged.last_mut() {
            Some(last) if start <= last.1 => last.1 = last.1.max(end),
            _ => merged.push((start, end)),
        }
    }

    let mut out = String::with_capacity(source.len());
    let mut ranges = merged.into_iter().peekable();
    for (i, ch) in source.char_indices() {
        while ranges.next_if(|(_, end)| *end <= i).is_some() {}
        let inside = ranges
            .peek()
            .is_some_and(|(start, end)| *start <= i && i < *end);
        if inside && ch != '\n' && ch != '\r' {
            // One space per UTF-16 unit, so columns are the unit the
            // editors use (an astral character counts twice).
            out.extend(std::iter::repeat_n(' ', ch.len_utf16()));
        } else {
            out.push(ch);
        }
    }
    out
}
