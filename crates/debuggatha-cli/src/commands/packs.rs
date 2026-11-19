//! `debuggatha packs`: the Review Packs the engine ships.

use debuggatha_engine::packs::builtin_packs;
use serde_json::json;

use crate::Context;
use crate::report::Format;

/// List the Review Packs the engine ships.
pub(crate) fn run(context: &Context) {
    let packs = builtin_packs();
    match context.format() {
        Format::Json => {
            let list: Vec<_> = packs
                .iter()
                .map(|p| {
                    json!({
                        "id": p.id, "version": p.version, "displayName": p.display_name,
                        "kind": p.kind, "rules": p.rules.len(),
                    })
                })
                .collect();
            println!(
                "{}",
                serde_json::to_string_pretty(&list).unwrap_or_default()
            );
        }
        Format::Text => {
            println!("Found {} Review Packs.", packs.len());
            for p in &packs {
                println!("- [{}] (v{}) {}", p.id, p.version, p.display_name);
            }
        }
    }
}
