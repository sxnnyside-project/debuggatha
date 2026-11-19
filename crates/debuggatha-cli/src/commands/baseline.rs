//! `debuggatha baseline`: record what the repository has today so later reviews report only what is new.

use std::collections::BTreeMap;

use clap::Subcommand;
use debuggatha_engine::analyzers::path_dirs;
use debuggatha_engine::baseline::{clear_baseline, load_baseline};
use debuggatha_engine::core::env::SystemEnv;
use debuggatha_engine::core::review_engine::ReviewDepth;
use debuggatha_engine::pipeline::{ReviewInput, Scope, create_baseline};
use serde_json::json;

use crate::commands::analyzers::AnalyzerArgs;
use crate::config;
use crate::report::{Format, describe_runs};
use crate::{Context, client_message};

#[derive(Subcommand)]
pub(crate) enum Command {
    /// Review the whole repository and record every finding as accepted.
    Create {
        /// Extra Review Packs, the same a review asks for.
        #[arg(long, num_args = 1..)]
        pack: Vec<String>,
        #[command(flatten)]
        analyzers: AnalyzerArgs,
    },
    /// Summarize the recorded baseline.
    Show,
    /// Delete the baseline; reviews report everything again.
    Clear,
}

pub(crate) fn run(context: &Context, command: &Command) -> Result<bool, String> {
    let root = context.root()?;
    match command {
        Command::Create { pack, analyzers } => {
            let loaded = config::load(&root)?;
            let (baseline, runs) = create_baseline(
                &ReviewInput {
                    root: &root,
                    scope: Scope::Workspace,
                    depth: ReviewDepth::Full,
                    pack_ids: loaded.config.packs.iter().chain(pack).cloned().collect(),
                    policy_id: None,
                    source_name: "debuggatha-cli",
                    persist: false,
                    include_baselined: true,
                    analyzers: Some(analyzers.options(&loaded.config)),
                    analyzer_search: path_dirs(),
                    semantic: None,
                },
                &SystemEnv,
            )
            .map_err(|e| client_message(&e))?;
            let findings: u32 = baseline.entries.iter().map(|e| e.count).sum();
            let files = baseline
                .entries
                .iter()
                .map(|e| e.file.as_str())
                .collect::<std::collections::BTreeSet<_>>()
                .len();
            match context.format() {
                Format::Json => println!(
                    "{}",
                    json!({ "status": "success", "file": ".debuggatha/baseline.json", "findings": findings, "analyzers": runs })
                ),
                Format::Text => {
                    for line in describe_runs(&runs) {
                        println!("{line}");
                    }
                    println!("Baseline recorded: {findings} findings across {files} files.");
                    println!(
                        "Commit .debuggatha/baseline.json. From now on `debuggatha review` reports only new findings; `--include-baselined` shows them all."
                    );
                }
            }
        }
        Command::Show => {
            let current = load_baseline(&root).map_err(|e| client_message(&e))?;
            match (current, context.format()) {
                (None, Format::Json) => {
                    println!("{}", json!({ "status": "success", "baseline": null }));
                }
                (None, Format::Text) => {
                    println!("No baseline. Create one with `debuggatha baseline create`.");
                }
                (Some(b), Format::Json) => {
                    let total: u32 = b.entries.iter().map(|e| e.count).sum();
                    println!(
                        "{}",
                        json!({ "status": "success", "baseline": b, "findings": total })
                    );
                }
                (Some(b), Format::Text) => {
                    let mut by_rule: BTreeMap<&str, u32> = BTreeMap::new();
                    for e in &b.entries {
                        *by_rule.entry(&e.rule_id).or_insert(0) += e.count;
                    }
                    let total: u32 = by_rule.values().sum();
                    println!("Baseline from {}: {total} accepted findings.", b.created_at);
                    let mut rules: Vec<_> = by_rule.into_iter().collect();
                    rules.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
                    for (rule, count) in rules {
                        println!("  {count:>5}  {rule}");
                    }
                }
            }
        }
        Command::Clear => {
            let removed = clear_baseline(&root).map_err(|e| client_message(&e))?;
            match context.format() {
                Format::Json => println!("{}", json!({ "status": "success", "removed": removed })),
                Format::Text if removed => println!("Baseline removed."),
                Format::Text => println!("There was no baseline."),
            }
        }
    }
    Ok(false)
}
