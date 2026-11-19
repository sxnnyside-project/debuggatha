use std::collections::BTreeMap;

use super::types::{Capability, CapabilityKind};
use crate::js::utf16_cmp;

fn label(kind: CapabilityKind) -> &'static str {
    match kind {
        CapabilityKind::Language => "Languages",
        CapabilityKind::Framework => "Frameworks",
        CapabilityKind::Platform => "Platforms",
        CapabilityKind::Tooling => "Tooling",
        CapabilityKind::Characteristic => "Characteristics",
    }
}

fn display_name(id: &str) -> String {
    match id {
        "typescript" => "TypeScript".to_owned(),
        "javascript" => "JavaScript".to_owned(),
        "nodejs" => "Node.js".to_owned(),
        "nextjs" => "Next.js".to_owned(),
        "aspnetcore" => "ASP.NET Core".to_owned(),
        "vscodeextension" | "vscode-extension" => "VS Code Extension".to_owned(),
        _ => {
            let mut chars = id.chars();
            chars.next().map_or_else(String::new, |first| {
                first.to_uppercase().collect::<String>() + chars.as_str()
            })
        }
    }
}

/// One line: what the repository is, by kind.
#[must_use]
pub fn summarize_capabilities(capabilities: &[Capability]) -> String {
    if capabilities.is_empty() {
        return "No capabilities detected.".to_owned();
    }
    let mut by_kind: BTreeMap<CapabilityKind, Vec<String>> = BTreeMap::new();
    for capability in capabilities {
        by_kind
            .entry(capability.kind)
            .or_default()
            .push(display_name(&capability.id));
    }
    let mut parts = Vec::new();
    // `CapabilityKind` derives `Ord` in the order the summary lists them.
    for (kind, mut names) in by_kind {
        names.sort_by(|a, b| utf16_cmp(a, b));
        names.dedup();
        parts.push(format!("{}: {}", label(kind), names.join(", ")));
    }
    parts.join(" — ")
}
