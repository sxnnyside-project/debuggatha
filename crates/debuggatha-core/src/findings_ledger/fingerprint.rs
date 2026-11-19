use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::review_engine::{Category, Evidence, Finding, rule_id_from_evidence};

/// What finding matching keys off: never the line number alone.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FindingFingerprint {
    pub file: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rule_id: Option<String>,
    pub category: Category,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_anchor: Option<String>,
}

/// Lowercase hex SHA-256 of the UTF-8 bytes, like Node's `createHash('sha256').update(s).digest('hex')`.
#[must_use]
pub fn sha256_hex(input: &str) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    Sha256::digest(input.as_bytes())
        .iter()
        .fold(String::with_capacity(64), |mut out, byte| {
            out.push(char::from(HEX[usize::from(byte >> 4)]));
            out.push(char::from(HEX[usize::from(byte & 0xf)]));
            out
        })
}

fn content_anchor(evidence: &[Evidence]) -> Option<String> {
    evidence.iter().find_map(|item| match item {
        Evidence::Code {
            excerpt: Some(excerpt),
            ..
        } => Some(sha256_hex(excerpt)[..16].to_owned()),
        _ => None,
    })
}

/// A multi-location finding is fingerprinted on its first location only (accepted v1 simplification).
#[must_use]
pub fn compute_finding_fingerprint(finding: &Finding) -> FindingFingerprint {
    FindingFingerprint {
        file: finding
            .locations
            .first()
            .map(|l| l.file.clone())
            .unwrap_or_default(),
        rule_id: rule_id_from_evidence(&finding.evidence),
        category: finding.category.clone(),
        content_anchor: content_anchor(&finding.evidence),
    }
}
