//! What a detector reports in a file.

use debuggatha_core::review_engine::{Confidence, LineRange, Severity};

/// An exact replacement on the hit's line, for rules whose fix is mechanical.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HitEdit {
    /// 1-based, inclusive (UTF-16 units).
    pub start_column: u32,
    /// 1-based, just past the last replaced character.
    pub end_column: u32,
    /// The text that takes the place of the span.
    pub replacement: String,
    /// Whether the edit is safe to apply without reading it.
    pub safety: debuggatha_core::review_engine::EditSafety,
}

/// One hit a detector finds in a file. Lines are the file's real lines, because a detector always
/// sees the whole file (a diff review keeps only the hits on added lines).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hit {
    /// 1-based line in the file.
    pub line: u32,
    /// 1-based columns on that line, in UTF-16 units: where the match starts and just past its end.
    pub columns: LineRange,
    /// The flagged line, trimmed and with secrets redacted.
    pub excerpt: String,
    /// How sure the detector is: high = unambiguous syntax, medium = heuristic, low = weak signal.
    pub confidence: Confidence,
    /// Overrides the rule's default severity when this hit is more or less serious.
    pub severity: Option<Severity>,
    /// An exact edit, for fixes that are mechanical.
    pub edit: Option<HitEdit>,
}
