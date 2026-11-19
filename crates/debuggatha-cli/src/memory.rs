use clap::Subcommand;
use debuggatha_engine::core::env::SystemEnv;
use debuggatha_engine::core::repository_memory::{
    MemoryDetail, MemoryError, MemoryItem, MemoryStore, activate_memory, archive_memory,
    confirm_memory, deprecate_memory, load_memory_store, reject_memory, remove_memory_item,
    save_memory_store, suggest_memory,
};
use serde_json::json;

use crate::report::Format;
use crate::{Context, client_message};

const ACTOR: &str = "cli";

#[derive(Subcommand)]
pub(crate) enum Command {
    /// List every remembered item.
    List {
        /// Only items in this lifecycle status.
        #[arg(long)]
        status: Option<String>,
    },
    /// Surface a detected item for review (detected -> suggested).
    Suggest { id: String },
    /// Confirm a suggested item. Only a person does this; a detection never can.
    Confirm {
        id: String,
        #[arg(long)]
        comment: Option<String>,
    },
    /// Start applying a confirmed item to future reviews (confirmed -> active).
    Activate { id: String },
    /// Reject a detected or suggested item: it was never true (-> archived).
    Reject {
        id: String,
        #[arg(long)]
        comment: Option<String>,
    },
    /// Stop applying an active item without deleting its history.
    Deprecate {
        id: String,
        #[arg(long)]
        comment: Option<String>,
    },
    /// Archive an item for audit; it is never consulted again.
    Archive {
        id: String,
        #[arg(long)]
        comment: Option<String>,
    },
    /// Delete an item entirely; archiving keeps a record, this does not.
    Remove { id: String },
}

fn describe(item: &MemoryItem) -> String {
    let detail = match &item.detail {
        MemoryDetail::Convention { rule } => rule.clone(),
        MemoryDetail::Decision { decision, .. } => decision.clone(),
        MemoryDetail::ReviewHistory { note, .. } => note.clone(),
        MemoryDetail::AcceptedDeviation { rule_id, .. } => format!("rule \"{rule_id}\""),
        MemoryDetail::Suppression {
            fingerprint_file,
            fingerprint_rule_id,
            fingerprint_category,
        } => format!(
            "{fingerprint_file} / {}",
            fingerprint_rule_id
                .as_deref()
                .unwrap_or(fingerprint_category)
        ),
        MemoryDetail::Exception {
            exception_kind,
            rule_id,
            ..
        } => format!(
            "{}{}",
            serde_json::to_value(exception_kind)
                .ok()
                .and_then(|v| v.as_str().map(str::to_owned))
                .unwrap_or_default(),
            rule_id
                .as_ref()
                .map_or_else(String::new, |r| format!(" ({r})"))
        ),
    };
    format!(
        "[{}] ({}, {}) {}",
        item.id,
        item.status.as_str(),
        serde_json::to_value(item.confidence)
            .ok()
            .and_then(|v| v.as_str().map(str::to_owned))
            .unwrap_or_default(),
        detail
    )
}

pub(crate) fn run(context: &Context, command: &Command) -> Result<bool, String> {
    let root = context.root()?;
    let store = load_memory_store(&root, &SystemEnv).map_err(|e| client_message(&e))?;
    let env = &SystemEnv;
    let moved: Result<(MemoryStore, &String, &str), MemoryError> = match command {
        Command::List { status } => {
            let items: Vec<&MemoryItem> = store
                .items
                .iter()
                .filter(|i| status.as_deref().is_none_or(|s| i.status.as_str() == s))
                .collect();
            match context.format() {
                Format::Json => {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&items).unwrap_or_default()
                    );
                }
                Format::Text => {
                    println!("{} memory item(s).", items.len());
                    for item in items {
                        println!("- {}", describe(item));
                    }
                }
            }
            return Ok(false);
        }
        Command::Suggest { id } => {
            suggest_memory(&store, id, ACTOR, env).map(|s| (s, id, "suggested"))
        }
        Command::Confirm { id, comment } => {
            confirm_memory(&store, id, ACTOR, comment.clone(), env).map(|s| (s, id, "confirmed"))
        }
        Command::Activate { id } => {
            activate_memory(&store, id, ACTOR, env).map(|s| (s, id, "activated"))
        }
        Command::Reject { id, comment } => {
            reject_memory(&store, id, ACTOR, comment.clone(), env).map(|s| (s, id, "rejected"))
        }
        Command::Deprecate { id, comment } => {
            deprecate_memory(&store, id, ACTOR, comment.clone(), env).map(|s| (s, id, "deprecated"))
        }
        Command::Archive { id, comment } => {
            archive_memory(&store, id, ACTOR, comment.clone(), env).map(|s| (s, id, "archived"))
        }
        Command::Remove { id } => remove_memory_item(&store, id, env).map(|s| (s, id, "removed")),
    };
    let (updated, id, verb) = moved.map_err(|e| client_message(&e))?;
    save_memory_store(&updated).map_err(|e| client_message(&e))?;
    let item = updated.items.iter().find(|i| i.id == *id);
    match context.format() {
        Format::Json => println!("{}", json!({ "status": "success", "item": item })),
        Format::Text => println!(
            "Memory item {id} {verb}{}.",
            item.map_or_else(String::new, |i| format!(" - now \"{}\"", i.status.as_str()))
        ),
    }
    Ok(false)
}
