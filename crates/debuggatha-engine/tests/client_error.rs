use std::io::{Error as IoError, ErrorKind};
use std::path::PathBuf;

use debuggatha_engine::client_error::classify_error;
use debuggatha_engine::core::findings_ledger::LedgerError;
use debuggatha_engine::core::repository_intelligence::ScanError;
use debuggatha_engine::core::stored::StoreError;
use debuggatha_engine::git::GitError;

const FILESYSTEM: &str = "A filesystem error occurred while accessing the repository.";

#[test]
fn a_domain_error_is_relayed_as_written() {
    let error = LedgerError::EntryNotFound("abc".into());
    assert_eq!(
        classify_error(&error).client_message,
        "No ledger entry with id \"abc\"."
    );
    assert_eq!(
        classify_error(&GitError::NotARepository).client_message,
        GitError::NotARepository.to_string()
    );
}

#[test]
fn a_scan_error_that_names_a_path_is_replaced_and_still_logged() {
    let error = ScanError::Io {
        path: PathBuf::from("/Users/jane/secret/package.json"),
        source: IoError::from(ErrorKind::NotFound),
    };
    let classified = classify_error(&error);
    assert_eq!(classified.client_message, FILESYSTEM);
    assert!(classified.log_detail.contains("/Users/jane/secret"));
}

#[test]
fn a_bare_io_error_is_replaced() {
    let error = IoError::new(ErrorKind::PermissionDenied, "denied opening /Users/jane/x");
    let classified = classify_error(&error);
    assert_eq!(classified.client_message, FILESYSTEM);
    assert!(classified.log_detail.contains("/Users/jane/x"));
}

#[test]
fn a_store_error_is_replaced_only_when_the_file_could_not_be_reached() {
    let Err(unreadable) =
        debuggatha_engine::core::stored::read_if_exists(&std::env::temp_dir(), "ledger")
    else {
        panic!("a directory is not a file");
    };
    assert!(StoreError::is_io(&unreadable));
    assert_eq!(classify_error(&unreadable).client_message, FILESYSTEM);

    let Err(corrupt) =
        debuggatha_engine::core::stored::deserialize_store::<serde_json::Value>("{", "ledger", 1)
    else {
        panic!("not JSON");
    };
    assert!(!corrupt.is_io());
    assert!(
        classify_error(&corrupt)
            .client_message
            .starts_with("Cannot read ledger")
    );
}

#[test]
fn an_error_nobody_has_vetted_says_nothing() {
    #[derive(Debug)]
    struct Mine;
    impl std::fmt::Display for Mine {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("failed at /Users/jane/secret")
        }
    }
    impl std::error::Error for Mine {}

    let classified = classify_error(&Mine);
    assert_eq!(classified.client_message, "An unexpected error occurred.");
    assert_eq!(classified.log_detail, "failed at /Users/jane/secret");
}

#[test]
fn a_filesystem_cause_behind_an_unknown_wrapper_is_still_caught() {
    #[derive(Debug)]
    struct Wrapper(IoError);
    impl std::fmt::Display for Wrapper {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("wrapper")
        }
    }
    impl std::error::Error for Wrapper {
        fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
            Some(&self.0)
        }
    }
    let error = Wrapper(IoError::new(ErrorKind::NotFound, "/Users/jane/y"));
    let classified = classify_error(&error);
    assert_eq!(classified.client_message, FILESYSTEM);
    assert_eq!(classified.log_detail, "wrapper: /Users/jane/y");
}
