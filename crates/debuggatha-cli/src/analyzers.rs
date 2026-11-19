use clap::Args as ClapArgs;
use clap::builder::PossibleValuesParser;
use debuggatha_engine::analyzers::{ADAPTERS, AnalyzerOptions, Mode, inventory, path_dirs};
use serde_json::json;

use crate::Context;
use crate::config::{self, Config};
use crate::report::Format;

fn known_ids() -> Vec<&'static str> {
    ADAPTERS.iter().map(|a| a.id).collect()
}

/// The flags every command that runs a review takes, so a baseline and a review agree. Enabling
/// an analyzer that runs project code or uses the network is only ever done here, by the person.
#[derive(ClapArgs, Clone)]
pub(crate) struct AnalyzerArgs {
    /// Also run these analyzers, including ones that run project code or use the network.
    #[arg(long, num_args = 1.., value_parser = PossibleValuesParser::new(known_ids()))]
    analyzer: Vec<String>,
    /// Run only the built-in detectors.
    #[arg(long)]
    no_analyzers: bool,
    /// Seconds one analyzer may run.
    #[arg(long, value_parser = clap::value_parser!(u64).range(1..))]
    analyzer_timeout: Option<u64>,
}

impl AnalyzerArgs {
    /// What a run allows: the flags, then the repository's config for what a flag did not say. The
    /// config can only narrow (turn analyzers off, skip some, set a time limit); enabling stays
    /// with the flags.
    pub(crate) fn options(&self, config: &Config) -> AnalyzerOptions {
        if self.no_analyzers || (config.analyzers_off && self.analyzer.is_empty()) {
            return AnalyzerOptions {
                mode: Mode::Off,
                ..AnalyzerOptions::default()
            };
        }
        AnalyzerOptions {
            mode: Mode::Auto,
            enable: self.analyzer.clone(),
            disable: config.disable.clone(),
            timeout_seconds: self.analyzer_timeout.or(config.timeout_seconds),
        }
    }
}

#[derive(ClapArgs)]
pub(crate) struct Args {
    #[command(flatten)]
    analyzers: AnalyzerArgs,
}

pub(crate) fn run(context: &Context, args: &Args) -> Result<bool, String> {
    let root = context.root()?;
    let loaded = config::load(&root)?;
    let entries = inventory(
        &root,
        &args.analyzers.options(&loaded.config),
        &path_dirs(),
        ADAPTERS,
    );
    match context.format() {
        Format::Json => {
            let list: Vec<_> = entries
                .iter()
                .map(|e| {
                    json!({
                        "id": e.id, "name": e.name, "license": e.license, "homepage": e.homepage,
                        "covers": e.covers, "trust": e.trust, "applies": e.applies,
                        "installed": e.installed, "version": e.version,
                        "willRun": e.will_run, "reason": e.reason,
                    })
                })
                .collect();
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({ "analyzers": list })).unwrap_or_default()
            );
        }
        Format::Text => {
            for e in &entries {
                let state = if e.will_run {
                    "will run"
                } else if !e.applies {
                    "not used here"
                } else if e.installed {
                    "not run"
                } else {
                    "not installed"
                };
                let version = e
                    .version
                    .as_deref()
                    .map_or_else(String::new, |v| format!(" {v}"));
                println!("{}{version}  {state}  {} · {}", e.id, e.license, e.covers);
                match (&e.reason, e.installed, e.applies) {
                    (Some(reason), true, true) => println!("  {reason}"),
                    (_, false, true) => println!("  {}", e.homepage),
                    _ => {}
                }
            }
            println!(
                "Debuggatha runs these tools as separate processes; it does not include or install them. Each is under its own license."
            );
        }
    }
    Ok(false)
}
