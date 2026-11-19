//! The prompts a host can offer its user.

use serde_json::{Value, json};

pub(crate) fn prompt_text(name: &str, args: &Value) -> Option<String> {
    let first = "Call `review_changes`. It reviews what changed in the working tree, including new files and deletions.";
    let explain = "Call `explain_finding` with its `ledgerId`.";
    let loop_of = |step: &str| {
        [
            format!("1. {step}"),
            "2. Fix the code for each finding in `changes.introduced` (and `changes.reopened`). Use the recommendation: apply its `edit` when it has one (check the result when `safety` is `review`), otherwise follow its `example`. Call `explain_finding` with the finding's `ledgerId` if you need the reasoning.".to_owned(),
            "3. Call `review_changes` again. `changes.fixed` should now list what you fixed and `changes.introduced` should be empty. Repeat until it is, or until a finding needs a decision from the user.".to_owned(),
            "4. Only when the code is right and the rule is wrong for this case, accept the finding with `suppress_finding` and a real reason, or with the inline comment `explain_finding` gives you. Never accept a finding just to make a review quiet, and tell the user every finding you accepted and why.".to_owned(),
        ]
        .join("\n")
    };
    match name {
        "review-flow" => Some(format!(
            "Review the code you just changed with Debuggatha, and fix what it finds:\n\n{}\n\nFinish by summarizing what was introduced, what you fixed, and what you accepted.",
            loop_of(first)
        )),
        "fix-findings" => {
            let severity = args["severity"]
                .as_str()
                .map(|s| format!(", severity {s} and above"))
                .unwrap_or_default();
            Some(format!(
                "List the open findings with `list_findings` (status \"open\"{severity}), then fix them starting with the most severe. For each one:\n\n{}",
                loop_of(explain)
            ))
        }
        "triage-findings" => Some(
            "List the open findings with the `list_findings` tool (status \"open\"), then propose resolving or dismissing each with `update_finding`.".to_owned(),
        ),
        _ => None,
    }
}

pub(crate) fn prompts() -> Value {
    json!([
        { "name": "review-flow", "title": "Review and fix what you changed",
          "description": "After writing or editing code: review the changes, fix what the review finds, and verify the fixes." },
        { "name": "fix-findings", "title": "Fix the open findings",
          "description": "Work through the findings the ledger still has open, most severe first.",
          "arguments": [{ "name": "severity", "description": "Only findings of this severity and above (critical, high, medium or low).", "required": false }] },
        { "name": "triage-findings", "title": "Triage open findings",
          "description": "List the open findings and decide, one by one, whether to resolve or dismiss them." },
    ])
}
