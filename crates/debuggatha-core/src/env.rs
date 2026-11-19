//! The clock and the id source, injected so the domain stays deterministic under test.

use std::time::{SystemTime, UNIX_EPOCH};

/// The clock and the id source of the domain, so tests can fix both.
pub trait Env {
    /// An ISO 8601 UTC timestamp with millisecond precision, like JavaScript's `toISOString()`.
    fn now(&self) -> String;
    /// A fresh identifier (a UUID v4 in production).
    fn new_id(&self) -> String;
}

/// The real clock and UUID v4 identifiers.
pub struct SystemEnv;

impl Env for SystemEnv {
    fn now(&self) -> String {
        let ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX));
        iso_from_unix_ms(ms)
    }

    fn new_id(&self) -> String {
        uuid::Uuid::new_v4().to_string()
    }
}

/// Civil date from days since 1970-01-01 (Howard Hinnant's algorithm), valid for the whole `i64` range used here.
#[must_use]
pub fn iso_from_unix_ms(ms: i64) -> String {
    let secs = ms.div_euclid(1000);
    let millis = ms.rem_euclid(1000);
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}.{millis:03}Z",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

/// The inverse of [`iso_from_unix_ms`]: `YYYY-MM-DDTHH:MM:SS(.mmm)Z` in UTC to Unix milliseconds.
/// Anything else (an offset, a date alone) is `None`.
#[must_use]
pub fn unix_ms_from_iso(text: &str) -> Option<i64> {
    let b = text.as_bytes();
    let digits = |from: usize, len: usize| -> Option<i64> {
        let part = text.get(from..from + len)?;
        part.bytes()
            .all(|c| c.is_ascii_digit())
            .then(|| part.parse().ok())?
    };
    if b.len() < 20
        || b[4] != b'-'
        || b[7] != b'-'
        || b[10] != b'T'
        || b[13] != b':'
        || b[16] != b':'
    {
        return None;
    }
    let (year, month, day) = (digits(0, 4)?, digits(5, 2)?, digits(8, 2)?);
    let (hour, minute, second) = (digits(11, 2)?, digits(14, 2)?, digits(17, 2)?);
    let (millis, rest) = if b[19] == b'.' {
        (digits(20, 3)?, 23)
    } else {
        (0, 19)
    };
    if &text[rest..] != "Z" || !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    // Howard Hinnant's days_from_civil.
    let y = year - i64::from(month <= 2);
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let doy = (153 * (if month > 2 { month - 3 } else { month + 9 }) + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    Some(((days * 24 + hour) * 60 + minute) * 60_000 + second * 1000 + millis)
}

#[cfg(test)]
mod tests {
    use super::{iso_from_unix_ms, unix_ms_from_iso};

    #[test]
    fn reads_back_what_it_writes() {
        for ms in [0, -1, 951_782_400_000, 1_783_714_855_852, 4_102_444_800_000] {
            assert_eq!(unix_ms_from_iso(&iso_from_unix_ms(ms)), Some(ms));
        }
        assert_eq!(
            unix_ms_from_iso("2026-07-10T20:20:55Z"),
            Some(1_783_714_855_000)
        );
        assert_eq!(unix_ms_from_iso("2026-07-10T20:20:55+02:00"), None);
        assert_eq!(unix_ms_from_iso("2026-07-10"), None);
        assert_eq!(unix_ms_from_iso("2026-13-10T20:20:55.852Z"), None);
    }

    #[test]
    fn matches_javascript_to_iso_string() {
        // Values produced by `new Date(ms).toISOString()`.
        assert_eq!(iso_from_unix_ms(0), "1970-01-01T00:00:00.000Z");
        assert_eq!(
            iso_from_unix_ms(1_783_714_855_852),
            "2026-07-10T20:20:55.852Z"
        );
        assert_eq!(
            iso_from_unix_ms(951_782_400_000),
            "2000-02-29T00:00:00.000Z"
        );
        assert_eq!(iso_from_unix_ms(-1), "1969-12-31T23:59:59.999Z");
    }
}
