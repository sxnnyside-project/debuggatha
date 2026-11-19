use std::path::Path;

use super::cache::{
    CacheEntry, RepositoryContextCache, build_fingerprint, has_fingerprint_changed,
};
use super::capabilities::derive_capabilities;
use super::criteria::scan_criteria;
use super::dependencies::scan_dependencies;
use super::documentation::scan_documentation;
use super::fs::list_dir;
use super::stack::scan_stack;
use super::types::{DirListing, RepositoryContext, ScanError};
use super::understanding::derive_understanding;
use crate::env::Env;

/// The single entry point of repository intelligence: runs every scanner once and returns one
/// immutable `RepositoryContext`. With a cache, an unchanged repository is served without a rescan.
///
/// # Errors
/// An unreadable root or manifest, or a `package.json` that does not parse.
pub fn build_repository_context(
    root: &Path,
    env: &dyn Env,
    cache: Option<&mut RepositoryContextCache>,
) -> Result<RepositoryContext, ScanError> {
    let key = root.to_string_lossy().into_owned();
    if let Some(cache) = cache.as_deref()
        && let Some(entry) = cache.get(&key)
        && !has_fingerprint_changed(&entry.fingerprint)
    {
        return Ok(entry.context.clone());
    }

    let root_entries = list_dir(root)?;
    let root_listing = DirListing {
        dir: root.to_path_buf(),
        entries: root_entries.clone(),
    };

    let stack = scan_stack(root, &root_entries)?;
    let capabilities = derive_capabilities(root, &root_entries, &stack);
    let dependencies = scan_dependencies(&root_entries, &stack);
    let documentation = scan_documentation(root, &root_entries)?;
    let criteria = scan_criteria(root, &root_entries)?;
    let understanding = derive_understanding(
        &stack.scan.profile,
        &documentation.profile,
        &criteria.profile,
    );

    let context = RepositoryContext {
        root_dir: key.clone(),
        generated_at: env.now(),
        has_git: root.join(".git").exists(),
        stack: stack.scan.profile.clone(),
        capabilities,
        dependencies: dependencies.profile,
        documentation: documentation.profile,
        criteria: criteria.profile,
        understanding,
    };

    if let Some(cache) = cache {
        let files_read: Vec<_> = stack
            .scan
            .files_read
            .into_iter()
            .chain(dependencies.files_read)
            .chain(documentation.files_read)
            .chain(criteria.files_read)
            .collect();
        let dirs = std::iter::once(root_listing)
            .chain(stack.scan.dirs_listed)
            .chain(documentation.dirs_listed)
            .chain(criteria.dirs_listed)
            .collect();
        cache.set(
            &key,
            CacheEntry {
                context: context.clone(),
                fingerprint: build_fingerprint(dirs, &files_read),
            },
        );
    }
    Ok(context)
}
