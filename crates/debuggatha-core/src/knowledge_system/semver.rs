//! The version comparison packs use: `x.y.z` with a `^`, `~` or exact range. Not full semver:
//! pre-release and build suffixes after the patch number are ignored, as in the TypeScript engine.

use std::cmp::Ordering;

use crate::js::js_trim;

struct Parsed {
    major: f64,
    minor: f64,
    patch: f64,
}

/// `^(\d+)\.(\d+)\.(\d+)` on the trimmed text. Numbers are doubles, like JavaScript's `Number`.
fn parse(version: &str) -> Option<Parsed> {
    /// A run of ASCII digits at the start of `text`, and what follows it.
    fn number(text: &str) -> Option<(f64, &str)> {
        let digits = text.bytes().take_while(u8::is_ascii_digit).count();
        if digits == 0 {
            return None;
        }
        Some((text[..digits].parse::<f64>().ok()?, &text[digits..]))
    }
    let (major, rest) = number(js_trim(version))?;
    let (minor, rest) = number(rest.strip_prefix('.')?)?;
    let (patch, _) = number(rest.strip_prefix('.')?)?;
    Some(Parsed {
        major,
        minor,
        patch,
    })
}

/// Negative, zero or positive like the TypeScript function; an unparseable version sorts first.
#[must_use]
pub fn compare_versions(a: &str, b: &str) -> Ordering {
    match (parse(a), parse(b)) {
        (None, None) => Ordering::Equal,
        (None, Some(_)) => Ordering::Less,
        (Some(_), None) => Ordering::Greater,
        (Some(a), Some(b)) => a
            .major
            .total_cmp(&b.major)
            .then(a.minor.total_cmp(&b.minor))
            .then(a.patch.total_cmp(&b.patch)),
    }
}

/// Whether `version` satisfies `range`: `*` (or empty), `^x.y.z`, `~x.y.z` or an exact `x.y.z`.
#[must_use]
pub fn satisfies_range(version: &str, range: &str) -> bool {
    let range = js_trim(range);
    if range == "*" || range.is_empty() {
        return true;
    }
    let Some(have) = parse(version) else {
        return false;
    };
    if let Some(base_text) = range.strip_prefix('^') {
        let Some(base) = parse(base_text) else {
            return false;
        };
        return have.major.total_cmp(&base.major).is_eq()
            && compare_versions(version, base_text) != Ordering::Less;
    }
    if let Some(base_text) = range.strip_prefix('~') {
        let Some(base) = parse(base_text) else {
            return false;
        };
        return have.major.total_cmp(&base.major).is_eq()
            && have.minor.total_cmp(&base.minor).is_eq()
            && compare_versions(version, base_text) != Ordering::Less;
    }
    parse(range).is_some() && compare_versions(version, range).is_eq()
}
