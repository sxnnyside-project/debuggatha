//! Repository Memory: structured engineering decisions (accepted deviations, suppressions,
//! exceptions, conventions). Port of `core/repository-memory`.
//!
//! Only `active` items ever filter findings, and only a person confirms one.

mod api;
mod filter;
mod item;
mod lifecycle;
mod matching;
mod persistence;

pub use api::{
    CURRENT_SCHEMA_VERSION, MemoryError, MemoryStore, activate_memory, add_memory_item,
    archive_memory, confirm_memory, create_empty_memory_store, deprecate_memory, reactivate_memory,
    reject_memory, remove_memory_item, suggest_memory,
};
pub use filter::{
    FilterFindingsResult, SuppressedFinding, SuppressionReason, filter_suppressed_findings,
};
pub use item::{
    ExceptionKind, MemoryConfidence, MemoryDetail, MemoryEvidence, MemoryHistoryEvent, MemoryItem,
    MemoryOrigin, NewMemoryItem, create_memory_item, transition_memory,
};
pub use lifecycle::{MemoryStatus, can_transition_memory, is_active_memory_status};
pub use matching::{accepted_deviation_matches, exception_matches, suppression_matches};
pub use persistence::{
    deserialize_memory_store, load_memory_store, memory_file_path, save_memory_store,
    serialize_memory_store,
};
