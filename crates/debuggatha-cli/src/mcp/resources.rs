//! The read-only documents a host can attach: the repository and the ledger.

use serde_json::{Value, json};

pub(crate) struct Resource {
    pub(crate) uri: &'static str,
    pub(crate) name: &'static str,
    pub(crate) title: &'static str,
    pub(crate) description: &'static str,
    pub(crate) method: &'static str,
    /// Every page of a listing, not the first.
    all: bool,
}

impl Resource {
    pub(crate) fn params(&self) -> Value {
        if self.all {
            json!({ "limit": u32::MAX })
        } else {
            json!({})
        }
    }
}

pub(crate) const RESOURCES: [Resource; 4] = [
    Resource {
        uri: "debuggatha://repository/context",
        name: "repository-context",
        title: "Repository context",
        description: "Stack, dependencies, documentation, criteria, and understanding.",
        method: "repository/context",
        all: false,
    },
    Resource {
        uri: "debuggatha://repository/summary",
        name: "repository-summary",
        title: "Repository summary",
        description: "Stack, dependencies, criteria, and ledger totals.",
        method: "repository/summary",
        all: false,
    },
    Resource {
        uri: "debuggatha://ledger",
        name: "ledger",
        title: "Findings ledger",
        description: "Every finding the repository's ledger tracks.",
        method: "findings/list",
        all: true,
    },
    Resource {
        uri: "debuggatha://ledger/history",
        name: "ledger-history",
        title: "Findings lifecycle history",
        description: "Every lifecycle event across all findings, newest first.",
        method: "findings/history",
        all: false,
    },
];

pub(crate) fn resources() -> Value {
    Value::Array(
        RESOURCES
            .iter()
            .map(|r| {
                json!({ "uri": r.uri, "name": r.name, "title": r.title,
                        "description": r.description, "mimeType": "application/json" })
            })
            .collect(),
    )
}
