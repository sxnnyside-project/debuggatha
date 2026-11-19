use indexmap::IndexMap;

use super::types::{Evidence, StackSignal};
use crate::knowledge_system::locale_compare;

/// Collects the evidence for each detected value, then lists them in collation order.
#[derive(Default)]
pub(super) struct SignalCollector {
    signals: IndexMap<String, Vec<Evidence>>,
}

impl SignalCollector {
    pub(super) fn add(&mut self, value: &str, evidence: Evidence) {
        self.signals
            .entry(value.to_owned())
            .or_default()
            .push(evidence);
    }

    pub(super) fn to_vec(&self) -> Vec<StackSignal> {
        let mut out: Vec<StackSignal> = self
            .signals
            .iter()
            .map(|(value, evidence)| StackSignal {
                value: value.clone(),
                evidence: evidence.clone(),
            })
            .collect();
        out.sort_by(|a, b| locale_compare(&a.value, &b.value));
        out
    }
}
