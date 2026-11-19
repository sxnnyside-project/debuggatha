//! What an error may say to a client. The full text goes to the server's log; the client gets a
//! message only from an error type known to write none of the machine in it, and a fixed one for
//! anything that touched the filesystem or that this module does not know.

use std::error::Error;

use debuggatha_core::findings_ledger::LedgerError;
use debuggatha_core::knowledge_system::{PackValidationError, PolicyError};
use debuggatha_core::repository_intelligence::ScanError;
use debuggatha_core::repository_memory::MemoryError;
use debuggatha_core::review_engine::{EmptyFileScope, FindingError, IllegalTransition};
use debuggatha_core::stored::StoreError;

use crate::diff::review::DiffReviewError;
use crate::git::GitError;
use crate::pipeline::PipelineError;
use crate::syntax::QueryError;

const FILESYSTEM: &str = "A filesystem error occurred while accessing the repository.";
const UNEXPECTED: &str = "An unexpected error occurred.";

/// An error split into what a client may read and what only the log may.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassifiedError {
    /// Safe to send to a client.
    pub client_message: String,
    /// The error and its causes as they are; for the log, never for a client.
    pub log_detail: String,
}

/// Classify `error`: the client gets a message from an error type known to write none of the machine's paths, and everything else gets a generic one.
#[must_use]
pub fn classify_error(error: &(dyn Error + 'static)) -> ClassifiedError {
    ClassifiedError {
        client_message: client_message(error),
        log_detail: log_detail(error),
    }
}

fn client_message(error: &(dyn Error + 'static)) -> String {
    let mut link = Some(error);
    while let Some(current) = link {
        if touches_filesystem(current) {
            return FILESYSTEM.to_owned();
        }
        link = current.source();
    }
    if is_known_safe(error) {
        error.to_string()
    } else {
        UNEXPECTED.to_owned()
    }
}

fn touches_filesystem(error: &(dyn Error + 'static)) -> bool {
    error.is::<std::io::Error>()
        || matches!(
            error.downcast_ref::<ScanError>(),
            Some(ScanError::Io { .. })
        )
        || error
            .downcast_ref::<StoreError>()
            .is_some_and(StoreError::is_io)
}

/// Types whose text names ids, titles, states or repository-relative names, never a location on
/// this machine. A new error type is not listed until someone has read what it prints.
fn is_known_safe(error: &(dyn Error + 'static)) -> bool {
    error.is::<ScanError>()
        || error.is::<StoreError>()
        || error.is::<GitError>()
        || error.is::<DiffReviewError>()
        || error.is::<PipelineError>()
        || error.is::<QueryError>()
        || error.is::<LedgerError>()
        || error.is::<MemoryError>()
        || error.is::<PolicyError>()
        || error.is::<PackValidationError>()
        || error.is::<FindingError>()
        || error.is::<IllegalTransition>()
        || error.is::<EmptyFileScope>()
}

fn log_detail(error: &(dyn Error + 'static)) -> String {
    let mut text = error.to_string();
    let mut link = error.source();
    while let Some(cause) = link {
        text.push_str(": ");
        text.push_str(&cause.to_string());
        link = cause.source();
    }
    text
}
