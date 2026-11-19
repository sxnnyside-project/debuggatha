//! Policy resolution: a Review Pack's rules plus the repository's own criteria, assembled for one
//! review. Its id is deterministic (ADR-0005) and conflicts are recorded, never dropped (ADR-0003).

use std::collections::HashSet;
use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::KNOWLEDGE_SCHEMA_VERSION;
use super::collate::locale_compare;
use super::pack::{PackDependency, RuleScope};
use super::registry::CapabilityRegistry;
use crate::canonical::{canonical_stringify, short_hash};
use crate::findings_ledger::sha256_hex;
use crate::repository_intelligence::RepositoryContext;
use crate::review_engine::{Category, PackReference, Severity};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(
    tag = "kind",
    rename_all = "lowercase",
    rename_all_fields = "camelCase"
)]
pub enum PolicyRuleOrigin {
    Pack {
        pack_id: String,
        pack_version: String,
    },
    Criteria,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PolicyRule {
    pub id: String,
    pub statement: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<Category>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_severity: Option<Severity>,
    pub origin: PolicyRuleOrigin,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ConflictRecord {
    pub winning_rule_id: String,
    pub losing_rule_id: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReviewPolicy {
    pub id: String,
    pub rules: Vec<PolicyRule>,
    pub pack_refs: Vec<PackReference>,
    pub conflicts: Vec<ConflictRecord>,
}

/// What resolution needs from the repository: when the context was built, the ids of the detected
/// capabilities, and the criteria profile (whose `rules` are `{ id, description }`).
#[derive(Debug, Clone)]
pub struct ResolutionContext {
    pub generated_at: String,
    pub capability_ids: Vec<String>,
    pub criteria: Value,
}

impl ResolutionContext {
    /// What resolution reads from a repository context: when it was built, the ids of its
    /// capabilities, and its criteria profile.
    #[must_use]
    pub fn from_context(context: &RepositoryContext) -> Self {
        Self {
            generated_at: context.generated_at.clone(),
            capability_ids: context.capabilities.iter().map(|c| c.id.clone()).collect(),
            criteria: serde_json::to_value(&context.criteria).unwrap_or(Value::Null),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct PolicyRequest {
    pub requested_pack_ids: Vec<String>,
    pub requested_policy_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicyError {
    UnresolvedDependencies(Vec<PackDependency>),
    PolicyIdMismatch { requested: String, computed: String },
}

impl fmt::Display for PolicyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnresolvedDependencies(missing) => {
                let plural = if missing.len() == 1 { "y" } else { "ies" };
                let described: Vec<String> = missing
                    .iter()
                    .map(|d| match d {
                        PackDependency::Pack {
                            pack_id,
                            version_range,
                        } => {
                            format!("pack \"{pack_id}\" ({version_range})")
                        }
                        PackDependency::Skill { skill_id } => format!("skill \"{skill_id}\""),
                    })
                    .collect();
                write!(
                    f,
                    "Cannot resolve review policy: {} unresolved dependenc{plural}: {}",
                    missing.len(),
                    described.join(", ")
                )
            }
            Self::PolicyIdMismatch {
                requested,
                computed,
            } => write!(
                f,
                "Requested policy id \"{requested}\" does not match the id \"{computed}\" recomputed \
                 from the current repository state and requested pack ids — the repository's criteria or a \
                 pack's registered version may have changed since this policy was originally resolved."
            ),
        }
    }
}

impl std::error::Error for PolicyError {}

struct Candidate {
    id: String,
    statement: String,
    category: Option<Category>,
    default_severity: Option<Severity>,
    origin: PolicyRuleOrigin,
    contradicts: Vec<String>,
    /// Position of the rule's pack in the request; `usize::MAX` for criteria and unrequested packs.
    pack_request_index: usize,
}

/// `value.toLowerCase().replace(/[^a-z0-9]/g, '')`
fn normalize_capability_id(value: &str) -> String {
    value
        .to_lowercase()
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .collect()
}

fn rule_applies(scope: &RuleScope, capability_ids: &[String]) -> bool {
    let has = |wanted: &str| {
        let wanted = normalize_capability_id(wanted);
        capability_ids.contains(&wanted)
    };
    match scope {
        RuleScope::Always | RuleScope::Glob { .. } => true,
        RuleScope::RequiresFramework { framework } => has(framework),
        RuleScope::RequiresLanguage { language } => has(language),
    }
}

/// # Errors
/// `UnresolvedDependencies` when a requested pack or a dependency is not registered, and
/// `PolicyIdMismatch` when the request pins a policy id that the current state no longer yields.
pub fn resolve_policy(
    context: &ResolutionContext,
    request: &PolicyRequest,
    registry: &CapabilityRegistry,
) -> Result<ReviewPolicy, PolicyError> {
    let requested = &request.requested_pack_ids;
    let resolved = registry.resolve_dependencies(requested);
    if !resolved.missing.is_empty() {
        return Err(PolicyError::UnresolvedDependencies(resolved.missing));
    }
    let request_index = |pack_id: &str| {
        requested
            .iter()
            .position(|id| id == pack_id)
            .unwrap_or(usize::MAX)
    };

    let mut candidates: Vec<Candidate> = Vec::new();
    for pack in &resolved.resolved {
        for rule in &pack.rules {
            if !rule_applies(&rule.applies_to, &context.capability_ids) {
                continue;
            }
            candidates.push(Candidate {
                id: rule.id.clone(),
                statement: rule.statement.clone(),
                category: Some(rule.category.clone()),
                default_severity: Some(rule.default_severity),
                origin: PolicyRuleOrigin::Pack {
                    pack_id: pack.id.clone(),
                    pack_version: pack.version.clone(),
                },
                contradicts: rule.contradicts.clone().unwrap_or_default(),
                pack_request_index: request_index(&pack.id),
            });
        }
    }
    let criteria_rules = context.criteria.get("rules").and_then(Value::as_array);
    for rule in criteria_rules.into_iter().flatten() {
        let text = |key: &str| {
            rule.get(key)
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned()
        };
        candidates.push(Candidate {
            id: text("id"),
            statement: text("description"),
            category: None,
            default_severity: None,
            origin: PolicyRuleOrigin::Criteria,
            contradicts: Vec::new(),
            pack_request_index: usize::MAX,
        });
    }

    let (survivors, mut conflicts) = resolve_conflicts(&candidates);
    let mut rules: Vec<PolicyRule> = survivors
        .into_iter()
        .map(|c| PolicyRule {
            id: c.id.clone(),
            statement: c.statement.clone(),
            category: c.category.clone(),
            default_severity: c.default_severity,
            origin: c.origin.clone(),
        })
        .collect();
    rules.sort_by(|a, b| locale_compare(&a.id, &b.id));

    let contributing: HashSet<&str> = rules
        .iter()
        .filter_map(|r| match &r.origin {
            PolicyRuleOrigin::Pack { pack_id, .. } => Some(pack_id.as_str()),
            PolicyRuleOrigin::Criteria => None,
        })
        .collect();
    let mut pack_refs: Vec<PackReference> = resolved
        .resolved
        .iter()
        .filter(|p| contributing.contains(p.id.as_str()))
        .map(|p| PackReference {
            id: p.id.clone(),
            version: p.version.clone(),
        })
        .collect();
    pack_refs.sort_by(|a, b| locale_compare(&a.id, &b.id));

    let id = compute_policy_id(&pack_refs, context);
    if let Some(wanted) = &request.requested_policy_id
        && !wanted.is_empty()
        && *wanted != id
    {
        return Err(PolicyError::PolicyIdMismatch {
            requested: wanted.clone(),
            computed: id,
        });
    }
    conflicts.sort_by(|a, b| locale_compare(&a.losing_rule_id, &b.losing_rule_id));
    Ok(ReviewPolicy {
        id,
        rules,
        pack_refs,
        conflicts,
    })
}

fn resolve_conflicts(candidates: &[Candidate]) -> (Vec<&Candidate>, Vec<ConflictRecord>) {
    let mut losers: HashSet<usize> = HashSet::new();
    let mut conflicts = Vec::new();
    let mut seen: HashSet<(usize, usize)> = HashSet::new();
    let mut record = |a_idx: usize, b_idx: usize, reason: String| {
        if !seen.insert((a_idx.min(b_idx), a_idx.max(b_idx))) {
            return;
        }
        let (winner, loser, loser_idx) = decide_precedence(candidates, a_idx, b_idx);
        losers.insert(loser_idx);
        conflicts.push(ConflictRecord {
            winning_rule_id: winner.id.clone(),
            losing_rule_id: loser.id.clone(),
            reason,
        });
    };

    for (i, candidate) in candidates.iter().enumerate() {
        for contradicted in &candidate.contradicts {
            for (j, other) in candidates.iter().enumerate() {
                if i != j && other.id == *contradicted {
                    record(
                        i,
                        j,
                        format!(
                            "rule \"{}\" explicitly declares a contradiction with \"{contradicted}\"",
                            candidate.id
                        ),
                    );
                }
            }
        }
    }
    for (i, candidate) in candidates.iter().enumerate() {
        if !matches!(candidate.origin, PolicyRuleOrigin::Pack { .. }) {
            continue;
        }
        for (j, other) in candidates.iter().enumerate() {
            if i != j
                && matches!(other.origin, PolicyRuleOrigin::Criteria)
                && other.id == candidate.id
            {
                record(
                    i,
                    j,
                    format!(
                        "pack rule \"{}\" shares its id with a repository criteria rule of the same id",
                        candidate.id
                    ),
                );
            }
        }
    }
    let survivors = candidates
        .iter()
        .enumerate()
        .filter(|(i, _)| !losers.contains(i))
        .map(|(_, c)| c)
        .collect();
    (survivors, conflicts)
}

/// Fixed precedence: the repository's own criteria beat a pack; then the pack requested first;
/// then the lower rule id.
fn decide_precedence(
    candidates: &[Candidate],
    a_idx: usize,
    b_idx: usize,
) -> (&Candidate, &Candidate, usize) {
    let (a, b) = (&candidates[a_idx], &candidates[b_idx]);
    let a_wins = match (&a.origin, &b.origin) {
        (PolicyRuleOrigin::Criteria, PolicyRuleOrigin::Pack { .. }) => true,
        (PolicyRuleOrigin::Pack { .. }, PolicyRuleOrigin::Criteria) => false,
        _ if a.pack_request_index != b.pack_request_index => {
            a.pack_request_index < b.pack_request_index
        }
        _ => locale_compare(&a.id, &b.id).is_le(),
    };
    if a_wins { (a, b, b_idx) } else { (b, a, a_idx) }
}

fn compute_policy_id(pack_refs: &[PackReference], context: &ResolutionContext) -> String {
    let mut refs: Vec<&PackReference> = pack_refs.iter().collect();
    refs.sort_by(|a, b| {
        locale_compare(&a.id, &b.id).then_with(|| locale_compare(&a.version, &b.version))
    });
    short_hash(&json!({
        "schemaVersion": KNOWLEDGE_SCHEMA_VERSION,
        "packRefs": refs,
        "repositoryGeneratedAt": context.generated_at,
        "criteriaFingerprint": sha256_hex(&canonical_stringify(&context.criteria)),
    }))
}
