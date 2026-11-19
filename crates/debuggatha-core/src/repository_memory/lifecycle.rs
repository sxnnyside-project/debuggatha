use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "lowercase")]
pub enum MemoryStatus {
    Detected,
    Suggested,
    Confirmed,
    Active,
    Deprecated,
    Archived,
}

impl MemoryStatus {
    pub const ALL: [MemoryStatus; 6] = [
        Self::Detected,
        Self::Suggested,
        Self::Confirmed,
        Self::Active,
        Self::Deprecated,
        Self::Archived,
    ];

    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Detected => "detected",
            Self::Suggested => "suggested",
            Self::Confirmed => "confirmed",
            Self::Active => "active",
            Self::Deprecated => "deprecated",
            Self::Archived => "archived",
        }
    }
}

/// The single source of truth for legal transitions. Archived is terminal.
#[must_use]
pub fn can_transition_memory(from: MemoryStatus, to: MemoryStatus) -> bool {
    use MemoryStatus::{Active, Archived, Confirmed, Deprecated, Detected, Suggested};
    matches!(
        (from, to),
        (Detected, Suggested)
            | (Suggested, Confirmed)
            | (Confirmed | Deprecated, Active)
            | (Active, Deprecated)
            | (
                Detected | Suggested | Confirmed | Active | Deprecated,
                Archived
            )
    )
}

/// Only `active` items filter findings.
#[must_use]
pub fn is_active_memory_status(status: MemoryStatus) -> bool {
    status == MemoryStatus::Active
}

#[cfg(test)]
mod tests {
    use super::{MemoryStatus, can_transition_memory};

    #[test]
    fn transition_table_is_exactly_the_typescript_one() {
        let legal: Vec<(&str, &str)> = MemoryStatus::ALL
            .iter()
            .flat_map(|f| MemoryStatus::ALL.iter().map(move |t| (*f, *t)))
            .filter(|(f, t)| can_transition_memory(*f, *t))
            .map(|(f, t)| (f.as_str(), t.as_str()))
            .collect();
        assert_eq!(
            legal,
            vec![
                ("detected", "suggested"),
                ("detected", "archived"),
                ("suggested", "confirmed"),
                ("suggested", "archived"),
                ("confirmed", "active"),
                ("confirmed", "archived"),
                ("active", "deprecated"),
                ("active", "archived"),
                ("deprecated", "active"),
                ("deprecated", "archived"),
            ]
        );
    }
}
