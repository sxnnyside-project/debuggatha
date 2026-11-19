//! The excerpt a finding carries: the line, trimmed, with credentials that name their own format
//! redacted, and cut at 200 UTF-16 units. The ledger fingerprint is `sha256(excerpt)[..16]`, so
//! the text must stay stable: existing ledgers depend on it (ADR-0006, decision 8).

use std::sync::LazyLock;

use debuggatha_core::js::js_trim;
use regress::Regex;

/// Formats that identify themselves: a match is a credential, not a guess.
const KNOWN_TOKEN_PATTERNS: &[&str] = &[
    r"\b(?:AKIA|ASIA)[0-9A-Z]{16}\b",
    r"\bgh[pousr]_[A-Za-z0-9]{36,}\b",
    r"\bgithub_pat_[A-Za-z0-9_]{50,}\b",
    r"\bxox[baprs]-[A-Za-z0-9-]{10,}\b",
    r"\b(?:sk|rk)_live_[A-Za-z0-9]{16,}\b",
    r"\bAIza[0-9A-Za-z_-]{35}\b",
    r"-----BEGIN (?:[A-Z]+ )?PRIVATE KEY-----",
    r"\beyJ[A-Za-z0-9_-]{10,}\.eyJ[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}\b",
    r"\bSG\.[A-Za-z0-9_-]{22}\.[A-Za-z0-9_-]{43}\b",
];

#[allow(clippy::expect_used)] // constant patterns, proven by the tests
pub(crate) static KNOWN_TOKENS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    KNOWN_TOKEN_PATTERNS
        .iter()
        // debuggatha-ignore-next-line no-unwrap-expect -- constant pattern, proven by the tests
        .map(|p| Regex::new(p).expect("a valid pattern"))
        .collect()
});

/// Every known credential format replaced by `[redacted]`, one pattern after the other, so a
/// replacement can never be matched by a later pattern.
#[must_use]
pub fn redact_known_tokens(line: &str) -> String {
    let mut text = line.to_owned();
    for pattern in KNOWN_TOKENS.iter() {
        let mut out = String::with_capacity(text.len());
        let mut from = 0;
        for m in pattern.find_iter(&text) {
            out.push_str(&text[from..m.start()]);
            out.push_str("[redacted]");
            from = m.end();
        }
        out.push_str(&text[from..]);
        text = out;
    }
    text
}

/// The first 200 UTF-16 units of `text`. JavaScript's `slice` may cut an astral character in half
/// and keep a lone surrogate, which Node hashes as U+FFFD; Rust cannot hold one, so the half is
/// written as U+FFFD, which gives the same fingerprint (see `docs/PARITY.md`).
fn first_200_units(text: &str) -> String {
    let mut out = String::new();
    let mut units = 0;
    for ch in text.chars() {
        let n = ch.len_utf16();
        if units + n > 200 {
            if units < 200 {
                out.push('\u{FFFD}');
            }
            break;
        }
        out.push(ch);
        units += n;
    }
    out
}

/// The excerpt a finding carries: the line trimmed, known credential formats redacted, cut at 200 UTF-16 units.
#[must_use]
pub fn excerpt_of(raw_line: &str) -> String {
    first_200_units(&redact_known_tokens(js_trim(raw_line)))
}
