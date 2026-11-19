//! `debuggatha policies`: the policy a review would apply.

use debuggatha_engine::core::env::SystemEnv;
use debuggatha_engine::pipeline::assemble_policy;

use crate::report::Format;
use crate::{Context, client_message};

/// Show the Review Policy a review of this repository would assemble right now.
pub(crate) fn run(context: &Context) -> Result<bool, String> {
    let root = context.root()?;
    let (_, policy, _) =
        assemble_policy(&root, &[], None, &SystemEnv).map_err(|e| client_message(&e))?;
    match context.format() {
        Format::Json => println!(
            "{}",
            serde_json::to_string_pretty(&policy).unwrap_or_default()
        ),
        Format::Text => {
            println!(
                "Policy \"{}\" - {} rule(s) from {} pack(s).",
                policy.id,
                policy.rules.len(),
                policy.pack_refs.len()
            );
            for pack in &policy.pack_refs {
                println!("- [{}] v{}", pack.id, pack.version);
            }
            if !policy.conflicts.is_empty() {
                println!(
                    "{} rule conflict(s) resolved, each recorded with the rule that won.",
                    policy.conflicts.len()
                );
            }
            println!(
                "This policy is assembled from the repository's detected capabilities and every registered Review Pack; it is not a fixed catalog entry."
            );
        }
    }
    Ok(false)
}
