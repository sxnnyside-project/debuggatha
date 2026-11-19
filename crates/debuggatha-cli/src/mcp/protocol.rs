//! What every MCP message of this server says about itself: versions, instructions, JSON-RPC shapes.

use serde_json::{Value, json};

/// The protocol versions this server speaks, newest first. A client asking for one of them gets it.
pub(crate) const VERSIONS: [&str; 4] = ["2025-11-25", "2025-06-18", "2025-03-26", "2024-11-05"];

pub(crate) const INSTRUCTIONS: &str = "Debuggatha reviews code against the Review Packs that match the repository's stack, and every finding cites the rule and evidence behind it. After you write or edit code, call review_changes, fix what changes.introduced lists (each recommendation says how, sometimes with an exact edit), and call it again until nothing new is introduced; changes.fixed confirms what your fixes resolved. Use explain_finding for the reasoning behind a finding. Accept a finding with suppress_finding only when the code is right and the rule is wrong, always with a real reason, and tell the user. Review tools record findings in the repository's ledger unless persist is false. When the client has a model, semantic: true also has it read the changed code; what it raises is a labeled suspicion to check, never a verdict.";

pub(crate) fn rpc_error(id: &Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

pub(crate) fn rpc_result(id: &Value, result: &Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

/// What a daemon response says to an MCP client: the result as structured content and as text,
/// or the message of the error.
pub(crate) fn tool_result(response: &Value) -> Value {
    if response["ok"] == json!(true) {
        let result = &response["result"];
        json!({
            "content": [{ "type": "text", "text": result.to_string() }],
            "structuredContent": result,
        })
    } else {
        let message = response["error"]["message"]
            .as_str()
            .unwrap_or("The call failed.");
        json!({ "isError": true, "content": [{ "type": "text", "text": message }] })
    }
}
