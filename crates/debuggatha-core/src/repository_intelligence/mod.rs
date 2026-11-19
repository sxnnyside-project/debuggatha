//! Repository Intelligence: what a repository is, from its own files. Deterministic, no model, and
//! every claim carries evidence. Where it differs from the TypeScript scanners, `docs/PARITY.md` says so.

mod cache;
mod capabilities;
mod criteria;
mod dependencies;
mod documentation;
mod fs;
mod signals;
mod snapshot;
mod stack;
mod summary;
mod types;
mod understanding;

pub use cache::{
    CacheEntry, FileStamp, Fingerprint, RepositoryContextCache, build_fingerprint,
    has_fingerprint_changed,
};
pub use capabilities::derive_capabilities;
pub use criteria::scan_criteria;
pub use dependencies::scan_dependencies;
pub use documentation::scan_documentation;
pub use snapshot::build_repository_context;
pub use stack::{StackScan, scan_stack};
pub use summary::summarize_capabilities;
pub use types::{
    Capability, CapabilityKind, CapabilityOrigin, Confidence, CriteriaProfile, CriteriaRule,
    DependencyProfile, DirListing, DocumentationKind, DocumentationProfile, DocumentationSource,
    Evidence, OpenQuestion, PlatformTarget, RepositoryContext, ScanError, ScanResult, StackProfile,
    StackSignal, UnderstandingProfile, WorkspaceType,
};
pub use understanding::derive_understanding;
