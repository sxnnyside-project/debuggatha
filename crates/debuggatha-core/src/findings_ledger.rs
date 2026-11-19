//! The findings ledger: a persistent record of findings across reviews (port of `core/findings-ledger`).

mod api;
mod association;
mod entry;
mod fingerprint;
mod lifecycle;
mod matching;
mod persistence;
mod summary;
mod sync;

pub use api::{LedgerEntryFilter, get_entry, get_history, list_entries, update_finding_status};
pub use association::{association_for, snapshot_ref_for};
pub use entry::{
    CURRENT_SCHEMA_VERSION, HistoryEvent, Ledger, LedgerEntry, LedgerError, ReviewAssociation,
    SnapshotRef, TransitionOrigin, create_empty_ledger, create_ledger_entry, refresh_ledger_entry,
    transition_finding,
};
pub use fingerprint::{FindingFingerprint, compute_finding_fingerprint, sha256_hex};
pub use lifecycle::{LifecycleStatus, can_transition_finding, is_active_finding_status};
pub use matching::{MatchOutcome, default_finding_matcher};
pub use persistence::{
    deserialize_ledger, ledger_file_path, load_ledger, save_ledger, serialize_ledger,
};
pub use summary::{LedgerSummary, summarize_ledger};
pub use sync::{SyncReport, SyncScope, synchronize_review_result};
