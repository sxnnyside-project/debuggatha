//! The review domain. Knows nothing about adapters, I/O or parsers.
//!
//! Each module gives what `packages/core` gives; the differential tests in `tests/` prove it.

pub mod canonical;
pub mod contract;
pub mod env;
pub mod findings_ledger;
pub mod js;
pub mod knowledge_system;
pub mod repository_intelligence;
pub mod repository_memory;
pub mod review_engine;
pub mod stored;
