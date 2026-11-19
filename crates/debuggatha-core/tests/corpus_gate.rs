//! The gate on real repositories: for each of the 61 repositories of the corpus, the Rust
//! engine must derive the same repository context and the same policy (rules, conflicts and id)
//! as the TypeScript engine did (`corpus/golden/ts-context`).
//!
//! The clones are not in the repository. To run the gate:
//!
//! ```text
//! node scripts/parity/fetch-corpus.mjs /tmp/debuggatha-corpus
//! DEBUGGATHA_CORPUS_DIR=/tmp/debuggatha-corpus cargo test -p debuggatha-core --test corpus_gate
//! ```
//!
//! Without `DEBUGGATHA_CORPUS_DIR` the test passes without checking anything and says so.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)] // a failed expect is a failed test

mod common;

use std::path::PathBuf;

use common::{CountingEnv, fixture, ignore_manifest_list, sort_unordered_evidence};
use debuggatha_core::knowledge_system::{
    CapabilityRegistry, PolicyRequest, ResolutionContext, ReviewPack, resolve_policy,
};
use debuggatha_core::repository_intelligence::{
    RepositoryContext, build_repository_context, derive_understanding,
};
use serde_json::{Value, json};

#[test]
#[ignore = "needs the corpus clones: run it with its `just corpus-*` recipe"]
fn the_corpus_gives_the_same_context_and_policy() {
    let Ok(corpus) = std::env::var("DEBUGGATHA_CORPUS_DIR") else {
        panic!("DEBUGGATHA_CORPUS_DIR must be set: run it with its `just corpus-*` recipe");
    };
    let golden = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../corpus/golden/ts-context");
    let packs: Vec<ReviewPack> =
        serde_json::from_value(fixture("knowledge-conformance.json")["realPacks"].clone()).unwrap();
    let pack_ids: Vec<String> = packs.iter().map(|p| p.id.clone()).collect();
    let registry = CapabilityRegistry::new(packs, vec![]);

    let mut files: Vec<_> = std::fs::read_dir(&golden)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    files.sort();
    assert!(files.len() >= 60, "the golden contexts are missing");
    let (mut checked, mut failures) = (0, Vec::new());
    for file in files {
        let record: Value = serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
        let name = record["name"].as_str().unwrap().to_owned();
        let root = PathBuf::from(&corpus).join(&name);
        if !root.exists() {
            failures.push(format!("{name}: not cloned in {corpus}"));
            continue;
        }
        let env = CountingEnv::new("g");
        let context = match build_repository_context(&root, &env, None) {
            Ok(c) => without_newer_languages(c),
            Err(error) => {
                if record.get("error").is_none() {
                    failures.push(format!(
                        "{name}: Rust failed ({error}) where TypeScript did not"
                    ));
                }
                continue;
            }
        };
        let mut got = serde_json::to_value(&context).unwrap();
        got["rootDir"] = json!("<root>");
        let mut want = record["context"].clone();
        // The build time is not reproducible; the policy id below depends on it, so it is carried over.
        got["generatedAt"] = want["generatedAt"].clone();
        sort_unordered_evidence(&mut got);
        sort_unordered_evidence(&mut want);
        ignore_manifest_list(&mut got);
        ignore_manifest_list(&mut want);
        if got != want {
            failures.push(format!(
                "{name}: context differs ({})",
                first_difference(&got, &want)
            ));
            continue;
        }
        let mut resolution = ResolutionContext::from_context(&context);
        resolution.generated_at = record["context"]["generatedAt"]
            .as_str()
            .unwrap()
            .to_owned();
        let request = PolicyRequest {
            requested_pack_ids: pack_ids.clone(),
            requested_policy_id: None,
        };
        match resolve_policy(&resolution, &request, &registry) {
            Ok(policy) if serde_json::to_value(&policy).unwrap() == record["policy"] => {
                checked += 1;
            }
            Ok(policy) => failures.push(format!(
                "{name}: policy differs (rust id {}, typescript id {})",
                policy.id, record["policy"]["id"]
            )),
            Err(error) => failures.push(format!("{name}: policy failed: {error}")),
        }
    }
    assert!(
        failures.is_empty(),
        "{} repositories differ ({checked} match):\n{}",
        failures.len(),
        failures.join("\n")
    );
    assert!(checked >= 60, "only {checked} repositories were compared");
}

/// Swift, Elixir, C++ and C# are read from their manifests; the TypeScript scanners never were
/// (`docs/PARITY.md`), so they are left out of what is compared.
fn without_newer_languages(mut context: RepositoryContext) -> RepositoryContext {
    const IDS: [&str; 4] = ["swift", "elixir", "cpp", "csharp"];
    const FRAMEWORKS: [&str; 6] = [
        "django",
        "flask",
        "rails",
        "spring",
        "phoenix",
        "aspnetcore",
    ];
    let newer = |manifest: &str| {
        [
            "Package.swift",
            "mix.exs",
            "CMakeLists.txt",
            ".csproj",
            ".sln",
        ]
        .iter()
        .any(|m| manifest.ends_with(m))
    };
    context
        .stack
        .languages
        .retain(|l| !IDS.contains(&l.value.to_lowercase().as_str()));
    context.stack.manifests.retain(|m| !newer(m));
    context
        .stack
        .frameworks
        .retain(|f| !FRAMEWORKS.contains(&f.value.to_lowercase().replace([' ', '.'], "").as_str()));
    context
        .capabilities
        .retain(|c| !IDS.contains(&c.id.as_str()) && !FRAMEWORKS.contains(&c.id.as_str()));
    context.understanding =
        derive_understanding(&context.stack, &context.documentation, &context.criteria);
    context
}

/// The first path at which two JSON values differ, to make a failure readable.
fn first_difference(got: &Value, want: &Value) -> String {
    fn walk(got: &Value, want: &Value, path: &str) -> Option<String> {
        match (got, want) {
            (Value::Object(left), Value::Object(right)) => {
                for key in left.keys().chain(right.keys()) {
                    match (left.get(key), right.get(key)) {
                        (Some(l), Some(r)) => {
                            if let Some(found) = walk(l, r, &format!("{path}.{key}")) {
                                return Some(found);
                            }
                        }
                        _ => return Some(format!("{path}.{key} is present on one side only")),
                    }
                }
                None
            }
            (Value::Array(left), Value::Array(right)) => {
                if left.len() != right.len() {
                    return Some(format!(
                        "{path} has {} items vs {}",
                        left.len(),
                        right.len()
                    ));
                }
                left.iter()
                    .zip(right)
                    .enumerate()
                    .find_map(|(i, (l, r))| walk(l, r, &format!("{path}[{i}]")))
            }
            _ if got == want => None,
            _ => Some(format!("{path}: {got} vs {want}")),
        }
    }
    walk(got, want, "").unwrap_or_else(|| "equal".to_owned())
}
