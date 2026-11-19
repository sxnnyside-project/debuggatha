use std::collections::BTreeMap;

use serde::Serialize;

use super::entry::Ledger;
use super::lifecycle::{LifecycleStatus, is_active_finding_status};
use crate::review_engine::Severity;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LedgerSummary {
    pub counts_by_status: BTreeMap<LifecycleStatus, u32>,
    pub active_by_severity: BTreeMap<Severity, u32>,
    pub active_by_category: BTreeMap<String, u32>,
}

#[must_use]
pub fn summarize_ledger(ledger: &Ledger) -> LedgerSummary {
    let mut counts_by_status: BTreeMap<_, _> =
        LifecycleStatus::ALL.iter().map(|s| (*s, 0)).collect();
    let mut active_by_severity: BTreeMap<_, _> = Severity::ALL.iter().map(|s| (*s, 0)).collect();
    let mut active_by_category = BTreeMap::new();
    for entry in &ledger.entries {
        *counts_by_status.entry(entry.status).or_insert(0) += 1;
        if is_active_finding_status(entry.status) {
            *active_by_severity
                .entry(entry.latest_finding.severity)
                .or_insert(0) += 1;
            *active_by_category
                .entry(entry.latest_finding.category.clone())
                .or_insert(0) += 1;
        }
    }
    LedgerSummary {
        counts_by_status,
        active_by_severity,
        active_by_category,
    }
}
