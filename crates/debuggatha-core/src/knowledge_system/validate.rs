//! Checks a pack against the Review Pack specification.

use std::collections::HashSet;
use std::fmt;

use super::pack::{PackDependency, ReviewPack};
use crate::js::js_trim;

/// A pack that does not meet the Review Pack specification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackValidationError {
    pack_id: String,
    /// What is wrong, one entry per problem.
    pub issues: Vec<String>,
}

impl fmt::Display for PackValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Review pack \"{}\" failed validation:\n- {}",
            self.pack_id,
            self.issues.join("\n- ")
        )
    }
}

impl std::error::Error for PackValidationError {}

/// `^\d+\.\d+\.\d+` on the trimmed version.
fn semver_shaped(version: &str) -> bool {
    let mut parts = js_trim(version).splitn(3, '.');
    let digits = |s: Option<&str>, last: bool| {
        s.is_some_and(|s| {
            let n = s.bytes().take_while(u8::is_ascii_digit).count();
            n > 0 && (last || n == s.len())
        })
    };
    let major = digits(parts.next(), false);
    let minor = digits(parts.next(), false);
    major && minor && digits(parts.next(), true)
}

/// Structural checks every pack must pass: unique ids, rules that belong to the pack and cite
/// knowledge that exists, and knowledge that states its limitations.
///
/// # Errors
/// All the issues found, in one error.
pub fn validate_review_pack(pack: &ReviewPack) -> Result<(), PackValidationError> {
    let mut issues: Vec<String> = Vec::new();
    if js_trim(&pack.id).is_empty() {
        issues.push("pack.id must be a non-empty string".to_owned());
    }
    if !semver_shaped(&pack.version) {
        issues.push(format!(
            "pack.version \"{}\" is not a semver-shaped string (expected \"x.y.z...\")",
            pack.version
        ));
    }
    let mut rule_ids = HashSet::new();
    for rule in &pack.rules {
        if !rule_ids.insert(rule.id.as_str()) {
            issues.push(format!(
                "duplicate rule id \"{}\" within pack \"{}\"",
                rule.id, pack.id
            ));
        }
        if rule.pack_id != pack.id {
            issues.push(format!(
                "rule \"{}\" declares packId \"{}\", which does not match its containing pack \"{}\"",
                rule.id, rule.pack_id, pack.id
            ));
        }
        if rule.knowledge_refs.is_empty() {
            issues.push(format!(
                "rule \"{}\" has no knowledgeRefs — every Rule must cite at least one KnowledgeEntry",
                rule.id
            ));
        }
        for reference in &rule.knowledge_refs {
            if !pack.knowledge.iter().any(|entry| entry.id == *reference) {
                issues.push(format!(
                    "rule \"{}\" cites unknown knowledgeRefs entry \"{reference}\"",
                    rule.id
                ));
            }
        }
    }
    let mut knowledge_ids = HashSet::new();
    for entry in &pack.knowledge {
        if !knowledge_ids.insert(entry.id.as_str()) {
            issues.push(format!(
                "duplicate knowledge entry id \"{}\" within pack \"{}\"",
                entry.id, pack.id
            ));
        }
        if entry.limitations.is_empty() {
            issues.push(format!(
                "knowledge entry \"{}\" has no limitations — every entry must document at least one known heuristic limitation (PACK_SPEC.md §4/§6)",
                entry.id
            ));
        }
    }
    for dependency in &pack.depends_on {
        issues.extend(describe_malformed_dependency(dependency));
    }
    if issues.is_empty() {
        Ok(())
    } else {
        Err(PackValidationError {
            pack_id: pack.id.clone(),
            issues,
        })
    }
}

/// "Both shapes at once" cannot be written in a typed pack, so it is refused when the JSON is read
/// instead (docs/PARITY.md); the empty and missing cases remain.
fn describe_malformed_dependency(dependency: &PackDependency) -> Vec<String> {
    match dependency {
        PackDependency::Pack {
            pack_id,
            version_range,
        } => {
            if pack_id.is_empty() {
                vec!["dependsOn entry declares neither a packId nor a skillId".to_owned()]
            } else if js_trim(version_range).is_empty() {
                vec![format!(
                    "dependsOn entry for pack \"{pack_id}\" is missing a non-empty versionRange"
                )]
            } else {
                vec![]
            }
        }
        PackDependency::Skill { skill_id } => {
            if skill_id.is_empty() {
                vec!["dependsOn entry declares neither a packId nor a skillId".to_owned()]
            } else {
                vec![]
            }
        }
    }
}
