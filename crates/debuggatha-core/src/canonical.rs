//! Canonical JSON, byte-identical to the TypeScript `JSON.stringify(sortKeysDeep(value))`.
//!
//! Things that differ between JavaScript and a naive Rust serializer, and are handled here:
//! keys sort by UTF-16 code unit (not by UTF-8 byte), numbers print the way `Number#toString`
//! does (`1e+21`, no `.0`; a hand-written formatter picked different digits for 1.2345678901234568e20), and integers beyond 2^53 are first rounded to a double.

use serde_json::{Number, Value};

use crate::findings_ledger::sha256_hex;

#[must_use]
pub fn canonical_stringify(value: &Value) -> String {
    let mut out = String::new();
    write_json(value, &mut out, true);
    out
}

/// Compact JSON the way `JSON.stringify` writes it. With `sort_keys` the keys are ordered like
/// `sortKeysDeep` does; without it they keep `Object.keys` order.
pub fn write_json(value: &Value, out: &mut String, sort_keys: bool) {
    write_value(value, out, sort_keys);
}

fn write_value(value: &Value, out: &mut String, sort: bool) {
    match value {
        Value::Null => out.push_str("null"),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Number(n) => out.push_str(&js_number(n)),
        // serde_json escapes like JSON.stringify: `"`, `\`, control characters as \b \f \n \r \t or \u00xx.
        Value::String(s) => out.push_str(&serde_json::to_string(s).unwrap_or_default()),
        Value::Array(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_value(item, out, sort);
            }
            out.push(']');
        }
        Value::Object(map) => {
            let mut keys = crate::js::js_keys(map);
            if sort {
                keys.sort_by(|a, b| a.encode_utf16().cmp(b.encode_utf16()));
            }
            out.push('{');
            for (i, key) in keys.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                out.push_str(&serde_json::to_string(key).unwrap_or_default());
                out.push(':');
                write_value(&map[*key], out, sort);
            }
            out.push('}');
        }
    }
}

/// `Number#toString` for the value JSON.parse would have produced.
fn js_number(n: &Number) -> String {
    const MAX_SAFE: u64 = 1 << 53;
    match (n.as_i64(), n.as_u64()) {
        (Some(i), _) if i.unsigned_abs() <= MAX_SAFE => return i.to_string(),
        (None, Some(u)) if u <= MAX_SAFE => return u.to_string(),
        _ => {}
    }
    js_f64(n.as_f64().unwrap_or(0.0))
}

fn js_f64(v: f64) -> String {
    // ECMAScript's Number-to-string, including its choice among equally short digit strings.
    ryu_js::Buffer::new().format_finite(v).to_owned()
}

/// First 16 hex characters of the SHA-256 of the canonical form, as the ledger uses for snapshots.
#[must_use]
pub fn short_hash(value: &Value) -> String {
    sha256_hex(&canonical_stringify(value))[..16].to_owned()
}
