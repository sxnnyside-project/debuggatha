//! Adoption mode. A repository never reviewed shows hundreds of findings on day one and nobody
//! starts fixing hundreds. A baseline records what is already there so reviews report only what is
//! new; the ledger still tracks everything, so nothing is forgotten, only quieted.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use debuggatha_core::env::Env;
use debuggatha_core::findings_ledger::compute_finding_fingerprint;
use debuggatha_core::js::utf16_cmp;
use debuggatha_core::review_engine::Finding;
use debuggatha_core::stored::{
    StoreError, deserialize_store, read_if_exists, remove_if_exists, serialize_store,
    write_atomically,
};
use serde::{Deserialize, Serialize};

const WHAT: &str = "baseline";
pub const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BaselineEntry {
    pub key: String,
    pub file: String,
    pub rule_id: String,
    pub title: String,
    /// How many identical findings (same file, rule and code) were accepted.
    pub count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Baseline {
    pub schema_version: u32,
    pub created_at: String,
    pub entries: Vec<BaselineEntry>,
}

/// A finding's identity for baselining: where it is, which rule, and the code it is about; never a
/// line number, so unrelated edits do not resurface it.
#[must_use]
pub fn baseline_key(finding: &Finding) -> String {
    let fingerprint = compute_finding_fingerprint(finding);
    format!(
        "{}::{}::{}",
        fingerprint.file,
        fingerprint.rule_id.unwrap_or_default(),
        fingerprint.content_anchor.unwrap_or_default()
    )
}

#[must_use]
pub fn baseline_path(root: &Path) -> PathBuf {
    root.join(".debuggatha").join("baseline.json")
}

/// `None` when the repository has no baseline.
///
/// # Errors
/// The file is not valid, has another schema version or fields this version does not know.
pub fn load_baseline(root: &Path) -> Result<Option<Baseline>, StoreError> {
    read_if_exists(&baseline_path(root), WHAT)?
        .map(|text| deserialize_store(&text, WHAT, SCHEMA_VERSION))
        .transpose()
}

#[must_use]
pub fn create_baseline_from(findings: &[Finding], env: &dyn Env) -> Baseline {
    let mut entries: BTreeMap<String, BaselineEntry> = BTreeMap::new();
    for finding in findings {
        let key = baseline_key(finding);
        entries
            .entry(key.clone())
            .and_modify(|e| e.count += 1)
            .or_insert_with(|| {
                let fingerprint = compute_finding_fingerprint(finding);
                BaselineEntry {
                    key,
                    file: fingerprint.file,
                    rule_id: fingerprint.rule_id.unwrap_or_default(),
                    title: finding.title.clone(),
                    count: 1,
                }
            });
    }
    let mut entries: Vec<BaselineEntry> = entries.into_values().collect();
    entries.sort_by(|a, b| utf16_cmp(&a.key, &b.key));
    Baseline {
        schema_version: SCHEMA_VERSION,
        created_at: env.now(),
        entries,
    }
}

/// # Errors
/// I/O failures; the partial file is removed.
pub fn save_baseline(root: &Path, baseline: &Baseline) -> Result<(), StoreError> {
    write_atomically(
        &baseline_path(root),
        &serialize_store(baseline, WHAT)?,
        WHAT,
    )
}

/// Whether there was a baseline to remove.
///
/// # Errors
/// The file exists and cannot be removed.
pub fn clear_baseline(root: &Path) -> Result<bool, StoreError> {
    remove_if_exists(&baseline_path(root), WHAT)
}

#[derive(Debug)]
pub struct Applied<'a> {
    pub fresh: Vec<&'a Finding>,
    pub baselined: Vec<&'a Finding>,
}

/// Splits findings into those the baseline already accepted and those that are new; identical
/// findings are matched one for one.
#[must_use]
pub fn apply_baseline<'a>(findings: &'a [Finding], baseline: &Baseline) -> Applied<'a> {
    let mut remaining: BTreeMap<&str, u32> = baseline
        .entries
        .iter()
        .map(|e| (e.key.as_str(), e.count))
        .collect();
    let mut applied = Applied {
        fresh: Vec::new(),
        baselined: Vec::new(),
    };
    for finding in findings {
        let key = baseline_key(finding);
        match remaining.get_mut(key.as_str()) {
            Some(left) if *left > 0 => {
                *left -= 1;
                applied.baselined.push(finding);
            }
            _ => applied.fresh.push(finding),
        }
    }
    applied
}
