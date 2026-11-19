use clap::Subcommand;
use debuggatha_engine::core::env::SystemEnv;
use debuggatha_engine::core::findings_ledger::{
    LedgerEntryFilter, LifecycleStatus, TransitionOrigin, get_entry, list_entries, load_ledger,
    save_ledger, summarize_ledger, update_finding_status,
};
use serde_json::json;

use crate::report::Format;
use crate::{Context, client_message};

#[derive(Subcommand)]
pub(crate) enum Command {
    /// List open findings.
    List,
    /// Show one finding and its history.
    Show { id: String },
    /// Mark a finding resolved.
    Resolve { id: String },
    /// Dismiss a finding.
    Dismiss { id: String },
    /// Reopen a resolved or dismissed finding.
    Reopen { id: String },
    /// Count findings by status, severity and category.
    Summary,
}

pub(crate) fn run(context: &Context, command: &Command) -> Result<bool, String> {
    let root = context.root()?;
    let ledger = load_ledger(&root, &SystemEnv).map_err(|e| client_message(&e))?;
    let json_out = matches!(context.format(), Format::Json);
    match command {
        Command::List => {
            let open = LedgerEntryFilter {
                status: Some(vec![LifecycleStatus::Open]),
                ..LedgerEntryFilter::default()
            };
            let entries = list_entries(&ledger, &open);
            if json_out {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&entries).unwrap_or_default()
                );
            } else {
                println!("Found {} open findings.", entries.len());
                for entry in entries {
                    let f = &entry.latest_finding;
                    println!(
                        "[{}] {:?} - {}: {}",
                        entry.id, f.severity, f.category, f.title
                    );
                }
            }
        }
        Command::Show { id } => {
            let entry = get_entry(&ledger, id).ok_or_else(|| format!("Finding {id} not found."))?;
            if json_out {
                println!(
                    "{}",
                    serde_json::to_string_pretty(entry).unwrap_or_default()
                );
            } else {
                let f = &entry.latest_finding;
                println!("Finding ID: {}", entry.id);
                println!("Status: {}", entry.status.as_str());
                println!("Severity: {:?}", f.severity);
                println!("Category: {}", f.category);
                println!("Summary: {}", f.title);
                if let Some(location) = f.locations.first() {
                    println!("Location: {}", location.file);
                }
                if !entry.history.is_empty() {
                    println!("History:");
                    for event in &entry.history {
                        println!(
                            "  - [{}] {}: {}",
                            event.timestamp,
                            event.new_state.as_str(),
                            event.comment.as_deref().unwrap_or("No comment")
                        );
                    }
                }
            }
        }
        Command::Resolve { id } => transition(context, &ledger, id, LifecycleStatus::Resolved)?,
        Command::Dismiss { id } => transition(context, &ledger, id, LifecycleStatus::Dismissed)?,
        Command::Reopen { id } => transition(context, &ledger, id, LifecycleStatus::Reopened)?,
        Command::Summary => {
            let summary = summarize_ledger(&ledger);
            if json_out {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&summary).unwrap_or_default()
                );
            } else {
                println!("By status:");
                for (status, count) in &summary.counts_by_status {
                    println!("  {}: {count}", status.as_str());
                }
                println!("Active, by severity:");
                for (severity, count) in &summary.active_by_severity {
                    println!("  {severity:?}: {count}");
                }
            }
        }
    }
    Ok(false)
}

fn transition(
    context: &Context,
    ledger: &debuggatha_engine::core::findings_ledger::Ledger,
    id: &str,
    to: LifecycleStatus,
) -> Result<(), String> {
    if get_entry(ledger, id).is_none() {
        return Err(format!("Finding {id} not found."));
    }
    let updated = update_finding_status(
        ledger,
        id,
        to,
        TransitionOrigin::Manual {
            actor: "cli".to_owned(),
        },
        Some(format!("CLI transition to {}", to.as_str())),
        &SystemEnv,
    )
    .map_err(|e| client_message(&e))?;
    save_ledger(&updated).map_err(|e| client_message(&e))?;
    match context.format() {
        Format::Json => println!(
            "{}",
            json!({ "status": "success", "entry": get_entry(&updated, id) })
        ),
        Format::Text => println!("Finding {id} transitioned to {}.", to.as_str()),
    }
    Ok(())
}
