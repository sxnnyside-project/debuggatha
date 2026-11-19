use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Reopened is a distinct state from open: it records that a closed finding came back.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "lowercase")]
pub enum LifecycleStatus {
    Open,
    Acknowledged,
    Resolved,
    Dismissed,
    Reopened,
}

impl LifecycleStatus {
    pub const ALL: [LifecycleStatus; 5] = [
        Self::Open,
        Self::Acknowledged,
        Self::Resolved,
        Self::Dismissed,
        Self::Reopened,
    ];

    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Acknowledged => "acknowledged",
            Self::Resolved => "resolved",
            Self::Dismissed => "dismissed",
            Self::Reopened => "reopened",
        }
    }
}

/// The single source of truth for legal transitions; nothing else encodes them.
#[must_use]
pub fn can_transition_finding(from: LifecycleStatus, to: LifecycleStatus) -> bool {
    use LifecycleStatus::{Acknowledged, Dismissed, Open, Reopened, Resolved};
    matches!(
        (from, to),
        (Open | Reopened, Acknowledged | Resolved | Dismissed)
            | (Acknowledged, Resolved | Dismissed)
            | (Resolved | Dismissed, Reopened)
    )
}

/// Entries in these statuses are candidates for re-matching against a new review's findings.
#[must_use]
pub fn is_active_finding_status(status: LifecycleStatus) -> bool {
    matches!(
        status,
        LifecycleStatus::Open | LifecycleStatus::Acknowledged | LifecycleStatus::Reopened
    )
}

#[cfg(test)]
mod tests {
    use super::{LifecycleStatus, can_transition_finding};

    #[test]
    fn transition_table_is_exactly_the_typescript_one() {
        let legal: Vec<(&str, &str)> = LifecycleStatus::ALL
            .iter()
            .flat_map(|f| LifecycleStatus::ALL.iter().map(move |t| (*f, *t)))
            .filter(|(f, t)| can_transition_finding(*f, *t))
            .map(|(f, t)| (f.as_str(), t.as_str()))
            .collect();
        assert_eq!(
            legal,
            vec![
                ("open", "acknowledged"),
                ("open", "resolved"),
                ("open", "dismissed"),
                ("acknowledged", "resolved"),
                ("acknowledged", "dismissed"),
                ("resolved", "reopened"),
                ("dismissed", "reopened"),
                ("reopened", "acknowledged"),
                ("reopened", "resolved"),
                ("reopened", "dismissed"),
            ]
        );
    }
}
