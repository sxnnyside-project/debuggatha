//! The lifecycle of a review session and the moves it allows.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The complete, enforced lifecycle of a review. `can_transition` is the single source of truth
/// for what is allowed; nothing outside this file encodes lifecycle logic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ReviewLifecycleStatus {
    /// Asked for; nothing has started.
    Requested,
    /// The repository was understood and the policy assembled.
    Prepared,
    /// Detectors and analyzers are running.
    Running,
    /// Finished with a result.
    Completed,
    /// Stopped with an error.
    Failed,
    /// Stopped because it was cancelled.
    Cancelled,
}

impl ReviewLifecycleStatus {
    /// Every status, in lifecycle order.
    pub const ALL: [Self; 6] = [
        Self::Requested,
        Self::Prepared,
        Self::Running,
        Self::Completed,
        Self::Failed,
        Self::Cancelled,
    ];

    /// The name used in reports and files.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Requested => "requested",
            Self::Prepared => "prepared",
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }

    const fn legal_targets(self) -> &'static [Self] {
        match self {
            Self::Requested => &[Self::Prepared, Self::Cancelled],
            Self::Prepared => &[Self::Running, Self::Failed, Self::Cancelled],
            Self::Running => &[Self::Completed, Self::Failed, Self::Cancelled],
            Self::Completed | Self::Failed | Self::Cancelled => &[],
        }
    }
}

/// Whether a session may move from one status to another.
#[must_use]
pub fn can_transition(from: ReviewLifecycleStatus, to: ReviewLifecycleStatus) -> bool {
    from.legal_targets().contains(&to)
}

/// Whether no further move is possible.
#[must_use]
pub fn is_terminal(status: ReviewLifecycleStatus) -> bool {
    status.legal_targets().is_empty()
}

#[cfg(test)]
mod tests {
    use super::ReviewLifecycleStatus::{
        Cancelled, Completed, Failed, Prepared, Requested, Running,
    };
    use super::{ReviewLifecycleStatus, can_transition, is_terminal};

    #[test]
    fn happy_path_and_early_exits() {
        assert!(can_transition(Requested, Prepared));
        assert!(can_transition(Prepared, Running));
        assert!(can_transition(Running, Completed));
        for from in [Prepared, Running] {
            assert!(can_transition(from, Failed));
            assert!(can_transition(from, Cancelled));
        }
        assert!(can_transition(Requested, Cancelled));
    }

    #[test]
    fn skipping_and_leaving_terminal_states_is_refused() {
        assert!(!can_transition(Requested, Running));
        assert!(!can_transition(Requested, Completed));
        assert!(!can_transition(Requested, Failed));
        assert!(!can_transition(Completed, Running));
        assert!(!can_transition(Failed, Running));
        assert!(!can_transition(Cancelled, Requested));
    }

    #[test]
    fn exactly_three_states_are_terminal() {
        let terminal: Vec<_> = ReviewLifecycleStatus::ALL
            .into_iter()
            .filter(|s| is_terminal(*s))
            .collect();
        assert_eq!(terminal, [Completed, Failed, Cancelled]);
    }

    #[test]
    fn wire_form_matches_as_str() {
        for s in ReviewLifecycleStatus::ALL {
            assert_eq!(serde_json::to_value(s).ok(), Some(s.as_str().into()));
        }
    }
}
