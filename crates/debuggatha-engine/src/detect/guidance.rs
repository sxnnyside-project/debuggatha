use std::collections::BTreeMap;
use std::sync::LazyLock;

use debuggatha_core::review_engine::RecommendationAction;
use serde::Deserialize;

/// What a finding tells the person (or agent) who has to act on it: why the pattern is a problem,
/// what to change, and the same code before and after. A rule with no entry falls back to the
/// pack's own statement.
#[derive(Debug, Clone, Deserialize)]
pub struct Guidance {
    pub why: String,
    pub action: RecommendationAction,
    pub fix: String,
    pub before: String,
    pub after: String,
}

// A file that does not parse is a build defect, and a test reads it; silence would hide every recommendation.
#[allow(clippy::expect_used)]
static GUIDANCE: LazyLock<BTreeMap<String, Guidance>> = LazyLock::new(|| {
    serde_json::from_str(include_str!("../../data/guidance.json"))
        .expect("data/guidance.json is valid")
});

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
