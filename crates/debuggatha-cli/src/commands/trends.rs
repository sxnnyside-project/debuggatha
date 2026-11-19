//! `debuggatha trends`: how the repository's findings change over time.

use std::collections::BTreeMap;

use clap::Args as ClapArgs;
use debuggatha_engine::core::env::{Env, SystemEnv, iso_from_unix_ms, unix_ms_from_iso};
use debuggatha_engine::core::findings_ledger::{
    LedgerEntry, LifecycleStatus, is_active_finding_status, load_ledger,
};
use serde::Serialize;
use serde_json::json;

use crate::report::Format;
use crate::{Context, client_message};

const DAY_MS: f64 = 86_400_000.0;
const WEEK_MS: i64 = 7 * 86_400_000;

#[derive(ClapArgs)]
pub(crate) struct Args {
    /// How far back to look, like 30d or 8w.
    #[arg(long, default_value = "30d", value_parser = parse_days)]
    since: u32,
}

fn parse_days(value: &str) -> Result<u32, String> {
    let (number, unit) = value
        .strip_suffix('w')
        .map_or((value.strip_suffix('d').unwrap_or(value), 1), |n| (n, 7));
    let days = number
        .parse::<u32>()
        .ok()
        .and_then(|n| n.checked_mul(unit))
        .filter(|d| *d >= 1);
    days.ok_or_else(|| "Expected a window of at least a day, like \"30d\" or \"8w\".".to_owned())
}

#[derive(Serialize)]
struct Week {
    start: String,
    introduced: u32,
    resolved: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Trends {
    days: u32,
    open: u32,
    open_by_severity: BTreeMap<String, u32>,
    weeks: Vec<Week>,
    introduced: u32,
    resolved: u32,
    mean_days_to_resolve: Option<f64>,
    top_files: Vec<(String, u32)>,
    top_rules: Vec<(String, u32)>,
}

fn tally(items: impl Iterator<Item = String>, limit: usize) -> Vec<(String, u32)> {
    let mut counts: BTreeMap<String, u32> = BTreeMap::new();
    for item in items {
        *counts.entry(item).or_insert(0) += 1;
    }
    let mut sorted: Vec<_> = counts.into_iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    sorted.truncate(limit);
    sorted
}

/// What the ledger's history says about how the findings are moving, over the last `days`.
fn compute(entries: &[LedgerEntry], days: u32, now: i64) -> Trends {
    let since = now - i64::from(days) * 86_400_000;
    let open: Vec<&LedgerEntry> = entries
        .iter()
        .filter(|e| is_active_finding_status(e.status))
        .collect();
    let mut open_by_severity = BTreeMap::new();
    for e in &open {
        *open_by_severity
            .entry(format!("{:?}", e.latest_finding.severity).to_lowercase())
            .or_insert(0) += 1;
    }
    let week_count = usize::try_from(days.div_ceil(7)).unwrap_or(1).max(1);
    let mut weeks: Vec<Week> = (0..week_count)
        .map(|i| Week {
            start: iso_from_unix_ms(since + i64::try_from(i).unwrap_or(0) * WEEK_MS)[..10]
                .to_owned(),
            introduced: 0,
            resolved: 0,
        })
        .collect();
    let week_of = |ms: i64| {
        usize::try_from(((ms - since) / WEEK_MS).max(0))
            .unwrap_or(0)
            .min(week_count - 1)
    };

    let (mut introduced, mut resolved) = (0, 0);
    let mut durations = Vec::new();
    for e in entries {
        let first = e
            .history
            .first()
            .and_then(|h| unix_ms_from_iso(&h.timestamp));
        if let Some(first) = first.filter(|f| *f >= since) {
            introduced += 1;
            weeks[week_of(first)].introduced += 1;
        }
        let closed = (e.status == LifecycleStatus::Resolved)
            .then(|| {
                e.history
                    .iter()
                    .rev()
                    .find(|h| h.new_state == LifecycleStatus::Resolved)
            })
            .flatten()
            .and_then(|h| unix_ms_from_iso(&h.timestamp));
        if let Some(closed) = closed.filter(|c| *c >= since) {
            resolved += 1;
            weeks[week_of(closed)].resolved += 1;
            if let Some(first) = first {
                #[allow(clippy::cast_precision_loss)] // a span of days, far below 2^53
                durations.push((closed - first) as f64 / DAY_MS);
            }
        }
    }
    #[allow(clippy::cast_precision_loss)] // a count of findings
    let mean_days_to_resolve =
        (!durations.is_empty()).then(|| durations.iter().sum::<f64>() / durations.len() as f64);

    Trends {
        days,
        open: u32::try_from(open.len()).unwrap_or(u32::MAX),
        open_by_severity,
        weeks,
        introduced,
        resolved,
        mean_days_to_resolve,
        top_files: tally(open.iter().map(|e| e.fingerprint.file.clone()), 5),
        top_rules: tally(
            open.iter().map(|e| {
                e.fingerprint
                    .rule_id
                    .clone()
                    .unwrap_or_else(|| format!("{:?}", e.fingerprint.category))
            }),
            5,
        ),
    }
}

pub(crate) fn run(context: &Context, args: &Args) -> Result<bool, String> {
    let root = context.root()?;
    let ledger = load_ledger(&root, &SystemEnv).map_err(|e| client_message(&e))?;
    let now = unix_ms_from_iso(&SystemEnv.now()).unwrap_or(0);
    let trends = compute(&ledger.entries, args.since, now);
    match context.format() {
        Format::Json => println!("{}", json!({ "status": "success", "trends": trends })),
        Format::Text if ledger.entries.is_empty() => println!(
            "The ledger is empty. Run `debuggatha review` (without --no-persist) to start recording findings."
        ),
        Format::Text => {
            let severities: Vec<String> = trends
                .open_by_severity
                .iter()
                .map(|(s, n)| format!("{n} {s}"))
                .collect();
            let by = if severities.is_empty() {
                String::new()
            } else {
                format!(": {}", severities.join(", "))
            };
            println!("{} open now{by}", trends.open);
            let fixed = trends.mean_days_to_resolve.map_or_else(String::new, |d| {
                format!(", fixed in {d:.1} days on average")
            });
            println!(
                "Last {} days: {} introduced, {} resolved{fixed}",
                trends.days, trends.introduced, trends.resolved
            );
            println!("\nweek of     introduced  resolved");
            for w in &trends.weeks {
                println!("{}  {:>10}  {:>8}", w.start, w.introduced, w.resolved);
            }
            if !trends.top_files.is_empty() {
                println!("\nmost open findings by file");
                for (file, n) in &trends.top_files {
                    println!("{n:>5}  {file}");
                }
                println!("by rule");
                for (rule, n) in &trends.top_rules {
                    println!("{n:>5}  {rule}");
                }
            }
        }
    }
    Ok(false)
}
