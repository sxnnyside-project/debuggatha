//! Contract types whose JSON Schema is generated from Rust (ADR-0006, decision 2).
//!
//! `generated_schema` is the one way a schema is produced from a Rust type. The files it owns in
//! `docs/schemas/` are checked for freshness by a test (`DEBUGGATHA_UPDATE_SCHEMAS=1 cargo test -p
//! debuggatha-core contract` rewrites them); the hand-reviewed schemas are checked against the
//! Rust enums below.

use schemars::JsonSchema;
use schemars::generate::SchemaSettings;
use serde_json::Value;

/// A draft-07 schema for `T`, closed like the published ones: every object with named properties
/// rejects unknown fields (stored files are strict; the wire is tolerant by the reader's rule).
///
/// # Panics
/// Never: a schema is always serializable.
#[must_use]
#[allow(clippy::expect_used)] // a schema is plain data; serialization cannot fail
pub fn generated_schema<T: JsonSchema>() -> Value {
    let generator = SchemaSettings::draft07().into_generator();
    let mut value =
        serde_json::to_value(generator.into_root_schema_for::<T>()).expect("serializes");
    close_objects(&mut value);
    drop_nulls_and_formats(&mut value);
    value
}

/// An absent optional field is how the stored files say "none" (`null` is refused by the strict
/// reader), so the schema must not offer `null`; and `uint32` is noise next to `minimum`.
fn drop_nulls_and_formats(value: &mut Value) {
    match value {
        Value::Object(map) => {
            let required: Vec<String> = map
                .get("required")
                .and_then(Value::as_array)
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_str().map(str::to_owned))
                        .collect()
                })
                .unwrap_or_default();
            if let Some(Value::Object(props)) = map.get_mut("properties") {
                for (name, prop) in props.iter_mut() {
                    if !required.contains(name) {
                        strip_null(prop);
                    }
                }
            }
            if matches!(map.get("format").and_then(Value::as_str), Some(f) if f.starts_with("uint") || f.starts_with("int"))
            {
                map.remove("format");
            }
            map.values_mut().for_each(drop_nulls_and_formats);
        }
        Value::Array(items) => items.iter_mut().for_each(drop_nulls_and_formats),
        _ => {}
    }
}

fn strip_null(prop: &mut Value) {
    let Value::Object(map) = prop else { return };
    if let Some(Value::Array(types)) = map.get_mut("type") {
        types.retain(|t| t != "null");
        if types.len() == 1 {
            let only = types.remove(0);
            map.insert("type".into(), only);
        }
    }
    if let Some(Value::Array(alts)) = map.get("anyOf")
        && let [one, none] = alts.as_slice()
        && none == &serde_json::json!({ "type": "null" })
    {
        let one = one.clone();
        map.remove("anyOf");
        if let Value::Object(inner) = one {
            map.extend(inner);
        }
    }
}

fn close_objects(value: &mut Value) {
    match value {
        Value::Object(map) => {
            if map.contains_key("properties") && !map.contains_key("additionalProperties") {
                map.insert("additionalProperties".into(), Value::Bool(false));
            }
            map.values_mut().for_each(close_objects);
        }
        Value::Array(items) => items.iter_mut().for_each(close_objects),
        _ => {}
    }
}

#[cfg(test)]
// A failed expect is a failed test, which is the intent here.
#[allow(clippy::expect_used)]
mod tests {
    use super::generated_schema;
    use crate::findings_ledger::Ledger;
    use crate::repository_intelligence::RepositoryContext;
    use crate::review_engine::{Confidence, Finding, ReviewRequest, ReviewSession, Severity};
    use schemars::schema_for;
    use serde_json::{Value, json};

    fn published(name: &str) -> Value {
        let text = include_str!("../../../docs/schemas/finding.schema.json");
        let schema: Value = serde_json::from_str(text).expect("finding.schema.json is JSON");
        schema["definitions"][name].clone()
    }

    fn values(schema: &Value) -> Value {
        // schemars 1 emits `enum` for unit variants; the published file uses `enum` too.
        schema["enum"].clone()
    }

    #[test]
    fn severity_matches_published_schema() {
        let generated = serde_json::to_value(schema_for!(Severity)).expect("serializes");
        assert_eq!(values(&generated), values(&published("Severity")));
    }

    #[test]
    fn confidence_matches_published_schema() {
        let generated = serde_json::to_value(schema_for!(Confidence)).expect("serializes");
        assert_eq!(values(&generated), values(&published("Confidence")));
    }

    /// `file`, schema: regenerate with `DEBUGGATHA_UPDATE_SCHEMAS=1`.
    fn owned() -> Vec<(&'static str, Value)> {
        vec![
            (
                "repository-context",
                generated_schema::<RepositoryContext>(),
            ),
            ("review-request", generated_schema::<ReviewRequest>()),
            ("review-session", generated_schema::<ReviewSession>()),
            ("finding", generated_schema::<Finding>()),
            ("ledger", generated_schema::<Ledger>()),
        ]
    }

    #[test]
    fn owned_schemas_are_current() {
        let update = std::env::var_os("DEBUGGATHA_UPDATE_SCHEMAS").is_some();
        for (name, schema) in owned() {
            let path = format!(
                "{}/../../docs/schemas/{name}.schema.json",
                env!("CARGO_MANIFEST_DIR")
            );
            let text = format!(
                "{}\n",
                serde_json::to_string_pretty(&schema).expect("serializes")
            );
            if update {
                std::fs::write(&path, &text).expect("writes");
            }
            let on_disk = std::fs::read_to_string(&path).unwrap_or_default();
            assert_eq!(
                on_disk, text,
                "{name}.schema.json is stale: run DEBUGGATHA_UPDATE_SCHEMAS=1 cargo test -p debuggatha-core contract"
            );
        }
    }

    #[test]
    fn wire_form_is_lowercase() {
        assert_eq!(
            serde_json::to_value(Severity::Informational).expect("ok"),
            json!("informational")
        );
    }
}
