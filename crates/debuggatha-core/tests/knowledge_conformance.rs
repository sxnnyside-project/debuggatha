//! Differential tests against the TypeScript knowledge system (ADR-0006, decision 8).
//!
//! `tests/fixtures/knowledge-conformance.json` is written by `scripts/parity/gen-knowledge-fixtures.ts` at tag `ts-engine-final`:
//! the 30 real Review Packs, three real repository contexts, and synthetic packs that exercise
//! conflicts, version ranges, skills and awkward rule ids.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)] // a failed expect is a failed test

mod common;

use std::cmp::Ordering;

use common::fixture;
use debuggatha_core::knowledge_system::{
    CapabilityRegistry, PolicyRequest, ResolutionContext, ReviewPack, SkillDescriptor,
    compare_versions, locale_compare, resolve_policy, satisfies_range, validate_review_pack,
};
use serde_json::{Value, json};

fn packs(value: &Value) -> Vec<ReviewPack> {
    serde_json::from_value(value.clone()).expect("packs parse")
}

fn registry_of(doc: &Value, set: &str) -> CapabilityRegistry {
    match set {
        "real" => CapabilityRegistry::new(packs(&doc["realPacks"]), vec![]),
        _ => CapabilityRegistry::new(
            packs(&doc["syntheticPacks"]),
            serde_json::from_value::<Vec<SkillDescriptor>>(doc["skills"].clone()).unwrap(),
        ),
    }
}

fn sign(ordering: Ordering) -> i64 {
    ordering as i64
}

#[test]
fn collation_matches_javascript_locale_compare() {
    let c = &fixture("knowledge-conformance.json")["collation"];
    let mut mismatches = Vec::new();
    for pair in c["pairs"].as_array().unwrap() {
        let (a, b) = (pair[0].as_str().unwrap(), pair[1].as_str().unwrap());
        if sign(locale_compare(a, b)) != pair[2].as_i64().unwrap() {
            mismatches.push(format!(
                "{a:?} vs {b:?}: js {} rust {}",
                pair[2],
                sign(locale_compare(a, b))
            ));
        }
    }
    assert!(
        mismatches.is_empty(),
        "{} of {} pairs differ:\n{}",
        mismatches.len(),
        c["pairs"].as_array().unwrap().len(),
        mismatches.join("\n")
    );
}

#[test]
fn semver_matches_the_typescript_functions() {
    let s = &fixture("knowledge-conformance.json")["semver"];
    for row in s["compare"].as_array().unwrap() {
        let (a, b) = (row[0].as_str().unwrap(), row[1].as_str().unwrap());
        assert_eq!(
            sign(compare_versions(a, b)),
            row[2].as_i64().unwrap(),
            "compare {a:?} {b:?}"
        );
    }
    for row in s["satisfies"].as_array().unwrap() {
        let (v, r) = (row[0].as_str().unwrap(), row[1].as_str().unwrap());
        assert_eq!(
            satisfies_range(v, r),
            row[2].as_bool().unwrap(),
            "satisfies {v:?} {r:?}"
        );
    }
}

#[test]
fn validation_reports_the_same_issues() {
    let doc = fixture("knowledge-conformance.json");
    let cases = doc["validate"].as_array().unwrap();
    assert!(cases.len() > 50);
    for case in cases {
        // A dependency that is both a pack and a skill cannot be written in a typed pack; none of
        // the fixtures needs it (docs/PARITY.md).
        let pack: ReviewPack = serde_json::from_value(case["pack"].clone()).expect("pack parses");
        let got = validate_review_pack(&pack).err().map(|e| e.to_string());
        assert_eq!(
            got.as_deref(),
            case["expected"].as_str(),
            "pack {}",
            case["pack"]["id"]
        );
    }
}

#[test]
fn registry_lookups_and_dependency_resolution_match() {
    let doc = fixture("knowledge-conformance.json");
    let registry = registry_of(&doc, "synthetic");
    for case in doc["registry"]["getPack"].as_array().unwrap() {
        let range = case["range"].as_str();
        let got = registry.get_pack("base", range).map(|p| p.version.clone());
        assert_eq!(got.as_deref(), case["expected"].as_str(), "range {range:?}");
    }
    for case in doc["registry"]["resolveDeps"].as_array().unwrap() {
        let ids: Vec<String> = serde_json::from_value(case["ids"].clone()).unwrap();
        let out = registry.resolve_dependencies(&ids);
        let resolved: Vec<String> = out
            .resolved
            .iter()
            .map(|p| format!("{}@{}", p.id, p.version))
            .collect();
        assert_eq!(json!(resolved), case["resolved"], "resolved for {ids:?}");
        assert_eq!(
            serde_json::to_value(&out.missing).unwrap(),
            case["missing"],
            "missing for {ids:?}"
        );
    }
}

#[test]
fn policies_are_identical_including_their_ids() {
    let doc = fixture("knowledge-conformance.json");
    let contexts = doc["contexts"].as_array().unwrap();
    let cases = doc["policies"].as_array().unwrap();
    assert!(cases.len() > 40);
    let mut checked_ids = 0;
    for case in cases {
        let ctx = contexts
            .iter()
            .find(|c| c["name"] == case["context"])
            .unwrap();
        let context = ResolutionContext {
            generated_at: ctx["generatedAt"].as_str().unwrap().to_owned(),
            capability_ids: serde_json::from_value(ctx["capabilityIds"].clone()).unwrap(),
            criteria: ctx["criteria"].clone(),
        };
        let request = PolicyRequest {
            requested_pack_ids: serde_json::from_value(case["requestedPackIds"].clone()).unwrap(),
            requested_policy_id: case["requestedPolicyId"].as_str().map(str::to_owned),
        };
        let registry = registry_of(&doc, case["set"].as_str().unwrap());
        let at = format!(
            "{} / {} / {}",
            case["context"], case["set"], case["requestedPackIds"]
        );
        match resolve_policy(&context, &request, &registry) {
            Ok(policy) => {
                assert_eq!(json!({ "policy": policy }), case["expected"], "{at}");
                checked_ids += 1;
            }
            Err(error) => assert_eq!(
                json!({ "error": error.to_string() }),
                case["expected"],
                "{at}"
            ),
        }
    }
    assert!(
        checked_ids > 30,
        "only {checked_ids} policies were compared"
    );
}

#[test]
fn every_real_pack_validates_and_round_trips() {
    let doc = fixture("knowledge-conformance.json");
    for original in doc["realPacks"].as_array().unwrap() {
        let pack: ReviewPack =
            serde_json::from_value(original.clone()).expect("a real pack parses");
        validate_review_pack(&pack).unwrap();
        assert_eq!(
            &serde_json::to_value(&pack).unwrap(),
            original,
            "pack {}",
            pack.id
        );
    }
}
