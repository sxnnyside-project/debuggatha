//! What a finding tells the person who has to act on it, from `data/guidance.json`.

use std::collections::BTreeMap;
use std::sync::LazyLock;

use debuggatha_core::review_engine::RecommendationAction;
use serde::Deserialize;

/// What a finding tells the person (or agent) who has to act on it: why the pattern is a problem,
/// what to change, and the same code before and after. A rule with no entry falls back to the
/// pack's own statement.
#[derive(Debug, Clone, Deserialize)]
pub struct Guidance {
    /// Why the pattern is a problem.
    pub why: String,
    /// What kind of change fixes it.
    pub action: RecommendationAction,
    /// The fix, in a sentence.
    pub fix: String,
    /// The code before the fix.
    pub before: String,
    /// The code after the fix.
    pub after: String,
}

// A file that does not parse is a build defect, and a test reads it; silence would hide every recommendation.
#[allow(clippy::expect_used)]
static GUIDANCE: LazyLock<BTreeMap<String, Guidance>> = LazyLock::new(|| {
    serde_json::from_str(include_str!("../../data/guidance.json"))
        // debuggatha-ignore-next-line no-unwrap-expect -- embedded table, proven by the tests
        .expect("data/guidance.json is valid")
});

/// The guidance of `rule_id`, when the rule has an entry.
#[must_use]
pub fn guidance_for(rule_id: &str) -> Option<&'static Guidance> {
    GUIDANCE.get(rule_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_loads_and_has_the_common_rules() {
        assert!(GUIDANCE.len() >= 20, "{} entries", GUIDANCE.len());
        for id in [
            "no-var",
            "no-explicit-any",
            "eqeqeq",
            "no-hardcoded-secrets",
        ] {
            assert!(guidance_for(id).is_some(), "{id}");
        }
        assert!(guidance_for("no-such-rule").is_none());
    }
}
