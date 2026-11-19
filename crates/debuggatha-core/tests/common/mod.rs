//! Helpers shared by the differential tests.
#![allow(dead_code, unreachable_pub, clippy::expect_used)] // each test binary uses a different subset

use std::cell::Cell;

use debuggatha_core::env::Env;
use serde_json::Value;

/// Ids are `n1`, `n2`, ... in creation order and time is constant, which is how the fixtures
/// were normalized.
pub struct CountingEnv {
    prefix: &'static str,
    next: Cell<u32>,
}

impl CountingEnv {
    pub fn new(prefix: &'static str) -> Self {
        Self {
            prefix,
            next: Cell::new(0),
        }
    }
}

impl Env for CountingEnv {
    fn now(&self) -> String {
        "T".to_owned()
    }
    fn new_id(&self) -> String {
        self.next.set(self.next.get() + 1);
        format!("{}{}", self.prefix, self.next.get())
    }
}

pub fn fixture(name: &str) -> Value {
    let path = format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    serde_json::from_str(&std::fs::read_to_string(path).expect("fixture exists")).expect("JSON")
}

/// The TypeScript engine lists `src/` with an unsorted `readdirSync`, so the order of the evidence
/// of `layered` and `feature-based` depends on the file system. Rust sorts it (docs/PARITY.md);
/// both sides are compared with that evidence in file order.
/// The reason of "What language(s)..." lists more manifests than the TypeScript text did
/// (`docs/PARITY.md`): the text is not compared.
pub fn ignore_manifest_list(context: &mut Value) {
    let Some(questions) = context["understanding"]["openQuestions"].as_array_mut() else {
        return;
    };
    for question in questions.iter_mut().filter(|q| {
        q["question"]
            .as_str()
            .is_some_and(|t| t.starts_with("What language"))
    }) {
        question["reason"] = Value::Null;
    }
}

pub fn sort_unordered_evidence(context: &mut Value) {
    let Some(capabilities) = context["capabilities"].as_array_mut() else {
        return;
    };
    for capability in capabilities {
        if matches!(capability["id"].as_str(), Some("layered" | "feature-based"))
            && let Some(evidence) = capability["evidence"].as_array_mut()
        {
            evidence.sort_by(|a, b| a["file"].as_str().cmp(&b["file"].as_str()));
        }
    }
}
