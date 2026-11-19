//! Group E: `no-hardcoded-secrets`, a text rule with two passes.
//!
//! 1. A credential that names its own format (`AKIA...`, `ghp_...`, a PEM header, a JWT...) is a
//!    credential wherever it is, even in a comment: found on the raw line.
//! 2. A string literal assigned to a name that says it is a secret (`password = "..."`), judged by
//!    the value (length, entropy, placeholders, words, URLs and paths are not secrets). The literals
//!    come from the syntax tree, so a quote inside a comment or a lifetime is not one.
//!
//! Test code carries fake credentials by design: pass 2 does not run there, and pass 1 reports a
//! known format as a smaller problem unless it looks synthetic, then not at all. The excerpt of a
//! pass 2 hit has the value replaced by `[redacted]`: secrets are redacted before they reach a
//! report, the ledger or a model.

use std::collections::BTreeSet;
use std::sync::LazyLock;

use regress::Regex;

use super::js::Lines;
use crate::detect::excerpt::{KNOWN_TOKENS, excerpt_of};
use crate::detect::hit::Hit;
use crate::detect::input::DetectorInput;
use crate::detect::runner::Registry;
use crate::scan_scope::is_test_path;
use crate::syntax::{Language, Span};
use debuggatha_core::review_engine::{Confidence, LineRange, Severity};

/// The rule this module answers for.
pub const RULE: &str = "no-hardcoded-secrets";

/// Registers the rule as a text detector: a credential has a format in any file, parsed or not.
pub fn register(registry: &mut Registry) {
    registry.register_text(RULE, detect);
}

#[allow(clippy::expect_used)] // constant patterns, proven by the tests
fn re(pattern: &str, flags: &str) -> Regex {
    // debuggatha-ignore-next-line no-unwrap-expect -- constant pattern, proven by the tests
    Regex::with_flags(pattern, flags).expect("a valid pattern")
}

static SECRET_NAME: LazyLock<Regex> = LazyLock::new(|| {
    re(
        r"(?:pass(?:word|wd)?|pwd|secret|token|api[_-]?key|apikey|access[_-]?key|private[_-]?key|credential|auth[_-]?key)",
        "i",
    )
});
static PASSWORD_NAME: LazyLock<Regex> =
    LazyLock::new(|| re(r"(?:pass(?:word|wd)?|pwd|secret)", "i"));
static PLACEHOLDER: LazyLock<Regex> = LazyLock::new(|| {
    re(
        r"^(?:[x*#.\-_]+)$|example|sample|placeholder|change[_-]?me|your[_-]|dummy|fake|mock|redacted|todo|<[^>]*>|\$\{|\{\{|%[sd]",
        "i",
    )
});
static ASSIGNED_NAME: LazyLock<Regex> =
    LazyLock::new(|| re(r#"([A-Za-z_][\w.-]*)["']?\s*(?:=>|[:=])\s*$"#, ""));
static NOT_A_SECRET_SHAPE: LazyLock<Regex> = LazyLock::new(|| re(r"^(?:https?:)?//", "i"));
static PATH_SHAPE: LazyLock<Regex> = LazyLock::new(|| re(r"^[./~]", ""));
static SEPARATORS: LazyLock<Regex> = LazyLock::new(|| re(r"[_.\-\s?\u{bf}()]+", "u"));
// Letters of any alphabet, so "Confirmar Contraseña" is not read as a secret.
static LETTERS: LazyLock<Regex> = LazyLock::new(|| re(r"^\p{L}+$", "u"));
// A character class followed by a quantifier is a regular expression, not a credential.
static REGEX_SHAPE: LazyLock<Regex> = LazyLock::new(|| re(r"\[[^\]]+\][+*?{]", ""));
// One validation rule of a framework: `required|string|current_password`, `max:255`.
static RULE_ITEM: LazyLock<Regex> = LazyLock::new(|| re(r"^[a-z][a-z0-9_]*(?::[^|]*)?$", ""));
static BASE64_BODY: LazyLock<Regex> = LazyLock::new(|| re(r"[A-Za-z0-9+/=]{40,}", ""));
static EXAMPLE_TOKEN: LazyLock<Regex> = LazyLock::new(|| re(r"EXAMPLE|SAMPLE|XXXX", "i"));
static FIXTURE_TOKEN: LazyLock<Regex> = LazyLock::new(|| {
    re(
        r"1234567890|ABCDEFG|dummy|fake|mock|fixture|synthetic|PRIVATE KEY|a1B2",
        "i",
    )
});
static FIXTURE_LINE: LazyLock<Regex> = LazyLock::new(|| {
    re(
        r"fixture|dummy|mock|fake|synthetic|placeholder|detect\(|expect\(",
        "i",
    )
});

fn utf16(text: &str) -> usize {
    text.encode_utf16().count()
}

fn column_of(line: &str, byte: usize) -> u32 {
    u32::try_from(utf16(&line[..byte])).unwrap_or(u32::MAX) + 1
}

fn entropy(value: &str) -> f64 {
    let mut counts = std::collections::BTreeMap::new();
    for c in value.chars() {
        *counts.entry(c).or_insert(0_u32) += 1;
    }
    let len = f64::from(u32::try_from(utf16(value)).unwrap_or(u32::MAX));
    counts
        .values()
        .map(|&n| f64::from(n) / len)
        .fold(0.0, |bits, p| bits - p * p.log2())
}

/// Words joined by separators (`access_token`, `Invalid password`) are labels, not secrets.
fn looks_like_words(value: &str) -> bool {
    let mut parts = Vec::new();
    let mut from = 0;
    for m in SEPARATORS.find_iter(value) {
        parts.push(&value[from..m.start()]);
        from = m.end();
    }
    parts.push(&value[from..]);
    parts.retain(|p| !p.is_empty());
    if parts.is_empty() || !parts.iter().all(|p| LETTERS.find(p).is_some()) {
        return false;
    }
    parts.len() > 1 || utf16(value) <= 24
}

/// A sentence of interface text: five or more words, each only letters once the punctuation around
/// it is gone. "If omitted, the application key is used" is a sentence; "shhhh, very secret" (three
/// words) stays a candidate, as it is a real example secret.
fn looks_like_prose(value: &str) -> bool {
    let words: Vec<&str> = value.split_whitespace().collect();
    words.len() >= 5
        && words.iter().all(|w| {
            let bare = w.trim_matches(|c: char| ".,;:!?\u{bf}\u{a1}\u{2026}()\"'".contains(c));
            LETTERS.find(bare).is_some()
        })
}

/// A password hash is not a credential: the secret it came from is what matters.
fn is_password_hash(value: &str) -> bool {
    [
        "$argon2", "$2a$", "$2b$", "$2y$", "$pbkdf2", "$scrypt$", "$5$", "$6$",
    ]
    .iter()
    .any(|p| value.starts_with(p))
}

/// Values that are not credentials although a secret-looking name holds them: a token prefix to
/// be completed (`base64url.bearer.phx.`), a command-line flag (`--password=`), a list of validation rules, a regular expression, a
/// string built with `$name` interpolation, as Dart and Kotlin do (`'$prefix$id'`).
fn is_not_a_credential(value: &str, interpolates: bool) -> bool {
    value.ends_with('.')
        || value.starts_with("--")
        || is_password_hash(value)
        || REGEX_SHAPE.find(value).is_some()
        || (value.contains('|') && value.split('|').all(|p| RULE_ITEM.find(p).is_some()))
        || (interpolates
            && value.split('$').skip(1).any(|t| {
                t.starts_with(['{', '_']) || t.starts_with(|c: char| c.is_ascii_alphabetic())
            }))
}

/// The text between the quotes of a literal, and the offset of the opening quote in it. A prefix
/// (`f"`, `r#"`, `b'`) and the `#`s closing a Rust raw string are not part of the value.
fn literal_value(text: &str) -> Option<&str> {
    let open = text.find(['"', '\'', '`'])?;
    let quote = text[open..].chars().next()?;
    let triple = format!("{quote}{quote}{quote}");
    let delimiter = if text[open..].starts_with(&triple) && text.len() >= open + 6 {
        3
    } else {
        1
    };
    let body = text[..text.len()]
        .trim_end_matches('#')
        .strip_suffix(&triple[..delimiter])
        .unwrap_or(text);
    body.get(open + delimiter..)
}

/// Values no secret-looking name makes a credential: too short, a placeholder, words, a sentence,
/// a URL or a path, or one of the shapes in [`is_not_a_credential`].
fn skipped_value(value: &str, interpolates: bool) -> bool {
    utf16(value) < 6
        || PLACEHOLDER.find(value).is_some()
        || looks_like_words(value)
        || looks_like_prose(value)
        || NOT_A_SECRET_SHAPE.find(value).is_some()
        || PATH_SHAPE.find(value).is_some()
        || is_not_a_credential(value, interpolates)
}

fn known_token_hits(
    input: &DetectorInput<'_>,
    tests: bool,
    flagged: &mut BTreeSet<usize>,
) -> Vec<(usize, Hit)> {
    let mut hits = Vec::new();
    let all_lines: Vec<&str> = input.content.split('\n').collect();
    for (index, line) in all_lines.iter().copied().enumerate() {
        for pattern in KNOWN_TOKENS.iter() {
            let Some(m) = pattern.find(line) else {
                continue;
            };
            let token = &line[m.start()..m.end()];
            if EXAMPLE_TOKEN.find(token).is_some() {
                continue;
            }
            // A PEM header alone (`append("-----BEGIN PRIVATE KEY-----\n")`) is code that writes a
            // key, not a key: it counts when a base64 body follows on this line or the next two.
            if token.starts_with("-----BEGIN") {
                let body = std::iter::once(&line[m.end()..])
                    .chain(all_lines.iter().skip(index + 1).take(2).copied())
                    .collect::<Vec<_>>()
                    .join("\n");
                if BASE64_BODY.find(&body).is_none() {
                    continue;
                }
            }
            if tests && (FIXTURE_TOKEN.find(token).is_some() || FIXTURE_LINE.find(line).is_some()) {
                continue;
            }
            let start = column_of(line, m.start());
            hits.push((
                index,
                Hit {
                    line: u32::try_from(index + 1).unwrap_or(u32::MAX),
                    columns: LineRange {
                        start,
                        end: start + u32::try_from(utf16(token)).unwrap_or(0),
                    },
                    excerpt: excerpt_of(line),
                    // In test code the same string is most likely a fixture: a smaller problem.
                    confidence: if tests {
                        Confidence::Medium
                    } else {
                        Confidence::High
                    },
                    severity: Some(if tests {
                        Severity::Medium
                    } else {
                        Severity::Critical
                    }),
                    edit: None,
                },
            ));
            flagged.insert(index);
            break;
        }
    }
    hits
}

fn assigned_literal_hits(
    input: &DetectorInput<'_>,
    flagged: &mut BTreeSet<usize>,
) -> Vec<(usize, Hit)> {
    let Some(syntax) = input.syntax else {
        return Vec::new();
    };
    let Some(facts) = syntax.facts.as_ref() else {
        return Vec::new();
    };
    let lines = Lines::new(input.content);
    let mut literals: Vec<Span> = facts.strings.clone();
    literals.sort_by_key(|s| s.start);
    let mut hits = Vec::new();
    for literal in literals {
        // The literal starts at its opening quote. One with a prefix (`f"`, `r"`, `b'`) is not
        // judged, as before: a raw string is mostly a character table (`_tokenChars = r"..."`).
        let literal_start = literal.start as usize;
        let Some(quote) = input.content[literal_start..literal.end as usize].find(['"', '\'', '`'])
        else {
            continue;
        };
        if quote > 0 {
            continue;
        }
        let start = literal_start;
        let line_number = lines.line(start);
        let index = line_number - 1;
        if flagged.contains(&index) {
            continue;
        }
        let raw = lines.raw_line(line_number);
        let line_start = lines.column(start) as usize - 1;
        // The literal's offset in its line, in bytes.
        let byte = raw
            .char_indices()
            .scan(0_usize, |units, (i, c)| {
                let here = *units;
                *units += c.len_utf16();
                Some((here, i))
            })
            .find(|(units, _)| *units == line_start)
            .map_or(raw.len(), |(_, i)| i);
        let before = &raw[..byte];
        let Some(name) = ASSIGNED_NAME
            .find(before)
            .and_then(|m| m.group(1))
            .map(|r| &before[r])
        else {
            continue;
        };
        if SECRET_NAME.find(name).is_none() {
            continue;
        }
        let Some(text) = input.content.get(start..literal.end as usize) else {
            continue;
        };
        let Some(value) = literal_value(text) else {
            continue;
        };
        if skipped_value(
            value,
            matches!(syntax.language, Language::Dart | Language::Kotlin),
        ) {
            continue;
        }
        let squashed: String = name
            .to_lowercase()
            .chars()
            .filter(|c| !matches!(c, '_' | '.' | '-'))
            .collect();
        if value.to_lowercase() == squashed {
            continue;
        }
        let strong = utf16(value) >= 16 && entropy(value) >= 3.0;
        if PASSWORD_NAME.find(name).is_none() && !strong {
            continue;
        }
        let first = column_of(raw, byte);
        hits.push((
            index,
            Hit {
                line: u32::try_from(line_number).unwrap_or(u32::MAX),
                columns: LineRange {
                    start: first,
                    end: first + u32::try_from(utf16(text)).unwrap_or(0),
                },
                // A value that runs over several lines is redacted where it starts: the first line holds
                // the start of the secret, and the old code left it in the excerpt.
                excerpt: excerpt_of(&raw.replacen(
                    value.split('\n').next().unwrap_or(value),
                    "[redacted]",
                    1,
                )),
                confidence: if strong {
                    Confidence::Medium
                } else {
                    Confidence::Low
                },
                severity: Some(Severity::High),
                edit: None,
            },
        ));
        flagged.insert(index);
    }
    hits
}

/// Runs the rule over one file.
#[must_use]
pub fn detect(input: &DetectorInput<'_>) -> Vec<Hit> {
    let tests = is_test_path(input.file);
    let mut flagged = BTreeSet::new();
    let mut hits = known_token_hits(input, tests, &mut flagged);
    if !tests {
        hits.extend(assigned_literal_hits(input, &mut flagged));
    }
    hits.sort_by_key(|(index, _)| *index);
    hits.into_iter().map(|(_, hit)| hit).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patterns_compile() {
        // Touching each forces the pattern to compile.
        std::hint::black_box((
            (
                &*SECRET_NAME,
                &*PASSWORD_NAME,
                &*PLACEHOLDER,
                &*ASSIGNED_NAME,
            ),
            (&*NOT_A_SECRET_SHAPE, &*PATH_SHAPE, &*SEPARATORS, &*LETTERS),
            (&*EXAMPLE_TOKEN, &*FIXTURE_TOKEN, &*FIXTURE_LINE),
            (&*REGEX_SHAPE, &*RULE_ITEM, &*BASE64_BODY),
        ));
    }

    #[test]
    fn every_hash_format_is_recognized_and_a_password_is_not() {
        for hash in [
            "$argon2id$v=19$m=65536",
            "$2a$12$abcdef",
            "$2b$12$abcdef",
            "$2y$12$abcdef",
            "$pbkdf2-sha256$29000$abc",
            "$scrypt$ln=16$abc",
            "$5$salt$abc",
            "$6$salt$abc",
        ] {
            assert!(is_password_hash(hash), "{hash}");
        }
        for plain in ["hunter2hunter2", "ab$6$cd", "$7$salt$abc", "$2c$12$abc"] {
            assert!(!is_password_hash(plain), "{plain}");
        }
    }

    #[test]
    fn prose_needs_five_words_and_only_letters() {
        assert!(looks_like_prose("If omitted, the application key is used"));
        assert!(looks_like_prose("Elige una contraseña segura y única"));
        assert!(!looks_like_prose("shhhh, very secret"));
        assert!(!looks_like_prose("one two three four"));
        assert!(!looks_like_prose("one two three four f1ve"));
    }

    #[test]
    fn values_lose_their_delimiters() {
        assert_eq!(literal_value("\"abc\""), Some("abc"));
        assert_eq!(literal_value("f'abc'"), Some("abc"));
        assert_eq!(literal_value("'''abc'''"), Some("abc"));
        assert_eq!(literal_value("r#\"abc\"#"), Some("abc"));
        assert_eq!(literal_value("`a${b}c`"), Some("a${b}c"));
    }
}
