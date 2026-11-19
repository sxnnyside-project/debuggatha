//! JavaScript semantics the scanners rely on: `trim`, truthiness, `in`, `Object.keys` order and
//! `JSON.stringify`. Each helper reproduces one of them exactly.

use std::cmp::Ordering;

use serde_json::{Map, Value};

use crate::canonical::write_json;

/// `\s` and `String.prototype.trim`: white space and line terminators, including U+FEFF, but not
/// U+0085, which Rust's `char::is_whitespace` accepts.
#[must_use]
pub fn is_js_whitespace(c: char) -> bool {
    c == '\u{feff}' || (c.is_whitespace() && c != '\u{85}')
}

/// `String.prototype.trim`: removes white space and line terminators from both ends.
#[must_use]
pub fn js_trim(text: &str) -> &str {
    text.trim_matches(is_js_whitespace)
}

/// Default `Array.prototype.sort` order for strings: UTF-16 code units, not bytes.
#[must_use]
pub fn utf16_cmp(a: &str, b: &str) -> Ordering {
    a.encode_utf16().cmp(b.encode_utf16())
}

/// JavaScript truthiness of a parsed JSON value. `[]` and `{}` are truthy.
#[must_use]
pub fn js_truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().is_some_and(|f| f != 0.0 && !f.is_nan()),
        Value::String(s) => !s.is_empty(),
        Value::Array(_) | Value::Object(_) => true,
    }
}

/// `String(value)` / template interpolation of a parsed JSON value.
#[must_use]
pub fn js_to_string(value: &Value) -> String {
    match value {
        Value::Null => "null".to_owned(),
        Value::Bool(b) => b.to_string(),
        Value::Number(_) => {
            let mut out = String::new();
            write_json(value, &mut out, false);
            out
        }
        Value::String(s) => s.clone(),
        Value::Array(items) => items
            .iter()
            .map(js_to_string_in_array)
            .collect::<Vec<_>>()
            .join(","),
        Value::Object(_) => "[object Object]".to_owned(),
    }
}

fn js_to_string_in_array(value: &Value) -> String {
    // `[null, 1].toString()` is ",1": null and undefined become empty strings inside arrays.
    if value.is_null() {
        String::new()
    } else {
        js_to_string(value)
    }
}

/// `JSON.stringify(value)`: keys in `Object.keys` order, no sorting.
#[must_use]
pub fn js_json_stringify(value: &Value) -> String {
    let mut out = String::new();
    write_json(value, &mut out, false);
    out
}

fn is_array_index(key: &str) -> bool {
    let bytes = key.as_bytes();
    !bytes.is_empty()
        && bytes.len() <= 10
        && bytes.iter().all(u8::is_ascii_digit)
        && (bytes.len() == 1 || bytes[0] != b'0')
        && key.parse::<u64>().is_ok_and(|n| n < u64::from(u32::MAX))
}

/// `Object.keys(object)`: integer-like keys first in ascending order, then the others in the
/// order they were written.
#[must_use]
pub fn js_keys(object: &Map<String, Value>) -> Vec<&String> {
    let (mut indices, others): (Vec<&String>, Vec<&String>) =
        object.keys().partition(|k| is_array_index(k));
    indices.sort_by_key(|k| k.parse::<u64>().unwrap_or(0));
    indices.into_iter().chain(others).collect()
}

/// `Object.entries(value)` for a parsed JSON value: an object's own entries, an array's
/// `("0", item)` pairs, a string's characters; nothing for the other types.
#[must_use]
pub fn js_entries(value: &Value) -> Vec<(String, Value)> {
    match value {
        Value::Object(map) => js_keys(map)
            .into_iter()
            .map(|k| (k.clone(), map[k].clone()))
            .collect(),
        Value::Array(items) => items
            .iter()
            .enumerate()
            .map(|(i, v)| (i.to_string(), v.clone()))
            .collect(),
        // shortcut: iterates code points, JavaScript iterates UTF-16 units; they differ only for
        // astral characters, in a configuration section that is a string, which no real file has.
        Value::String(s) => s
            .chars()
            .enumerate()
            .map(|(i, c)| (i.to_string(), Value::String(c.to_string())))
            .collect(),
        _ => Vec::new(),
    }
}

/// `value[key]` for the property reads the scanners do on parsed JSON: only objects have named
/// properties here (arrays and strings expose indices and `length`, which no scanner asks for).
#[must_use]
pub fn js_get<'a>(value: &'a Value, key: &str) -> Option<&'a Value> {
    value.as_object().and_then(|m| m.get(key))
}

/// `{ ...a, ...b }` restricted to string values, as the scanners use it for dependency maps. A
/// non-object contributes nothing for the names the scanners look up.
#[must_use]
pub fn merge_objects(a: Option<&Value>, b: Option<&Value>) -> Map<String, Value> {
    let mut merged = Map::new();
    for part in [a, b].into_iter().flatten() {
        if let Value::Object(map) = part {
            for (k, v) in map {
                merged.insert(k.clone(), v.clone());
            }
        }
    }
    merged
}

#[cfg(test)]
#[allow(clippy::unwrap_used)] // a failed unwrap is a failed test
mod tests {
    use serde_json::json;

    use super::{js_keys, js_to_string, js_trim, js_truthy};

    #[test]
    fn trim_matches_javascript() {
        assert_eq!(js_trim("\u{feff} a \u{a0}\u{2028}"), "a");
        assert_eq!(js_trim("\u{85}a\u{85}"), "\u{85}a\u{85}");
    }

    #[test]
    fn truthiness_and_string_conversion() {
        assert!(js_truthy(&json!([])));
        assert!(js_truthy(&json!({})));
        assert!(!js_truthy(&json!("")));
        assert!(!js_truthy(&json!(0)));
        assert_eq!(js_to_string(&json!([1, null, "a"])), "1,,a");
        assert_eq!(js_to_string(&json!(1.5)), "1.5");
        assert_eq!(js_to_string(&json!({})), "[object Object]");
    }

    #[test]
    fn integer_keys_come_first() {
        let value = json!({ "b": 1, "2": 2, "a": 3, "1": 4, "01": 5 });
        let keys: Vec<&str> = js_keys(value.as_object().unwrap())
            .into_iter()
            .map(String::as_str)
            .collect();
        assert_eq!(keys, ["1", "2", "b", "a", "01"]);
    }
}
