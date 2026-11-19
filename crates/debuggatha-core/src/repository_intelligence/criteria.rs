//! Criteria Resolution: the repository's own rules, from documentation, configuration and manifests.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use regress::Regex;
use serde_json::Value;

use super::fs::read_text;
use super::types::{CriteriaProfile, CriteriaRule, Evidence, ScanError, ScanResult};
use crate::js::{
    js_entries, js_get, js_json_stringify, js_to_string, js_trim, js_truthy, utf16_cmp,
};

/// The patterns in this file are literals checked by the tests, so a failure cannot happen.
#[allow(clippy::expect_used)]
fn regex(pattern: &str, flags: &str) -> Regex {
    // debuggatha-ignore-next-line no-unwrap-expect -- constant pattern, proven by the tests
    Regex::with_flags(pattern, flags).expect("a valid pattern")
}

fn rule(id: String, description: String, file: &str, detail: String) -> CriteriaRule {
    CriteriaRule {
        id,
        description,
        source: Evidence::new(file, detail),
    }
}

/// `value ?? {}` followed by a property read: only an object has the property.
fn section<'a>(value: &'a Value, key: &str) -> Option<&'a Value> {
    js_get(value, key).filter(|v| !v.is_null())
}

fn eslint_rules(json: &Value, file: &str) -> Vec<CriteriaRule> {
    let rules = section(json, "rules").cloned().unwrap_or(Value::Null);
    js_entries(&rules)
        .into_iter()
        .map(|(name, value)| {
            rule(
                format!("eslint:{name}"),
                format!("ESLint rule \"{name}\" configured"),
                file,
                js_json_stringify(&value),
            )
        })
        .collect()
}

fn biome_rules(json: &Value, file: &str) -> Vec<CriteriaRule> {
    let mut out = Vec::new();
    let linter_rules = section(json, "linter")
        .and_then(|l| section(l, "rules"))
        .cloned()
        .unwrap_or(Value::Null);
    for (category, category_rules) in js_entries(&linter_rules) {
        if category == "recommended" {
            let shown = js_to_string(&category_rules);
            out.push(rule(
                "biome:linter.recommended".to_owned(),
                format!("Biome recommended lint rules: {shown}"),
                file,
                format!("linter.rules.recommended = {shown}"),
            ));
            continue;
        }
        if js_truthy(&category_rules)
            && matches!(category_rules, Value::Object(_) | Value::Array(_))
        {
            for (name, value) in js_entries(&category_rules) {
                out.push(rule(
                    format!("biome:{category}.{name}"),
                    format!("Biome rule \"{category}.{name}\" configured"),
                    file,
                    js_json_stringify(&value),
                ));
            }
        }
    }
    let formatter = section(json, "formatter").cloned().unwrap_or(Value::Null);
    for (key, value) in js_entries(&formatter) {
        if key == "enabled" {
            continue;
        }
        let shown = js_json_stringify(&value);
        out.push(rule(
            format!("biome:formatter.{key}"),
            format!("Biome formatter setting \"{key}\" = {shown}"),
            file,
            format!("formatter.{key} = {shown}"),
        ));
    }
    out
}

fn prettier_rules(json: &Value, file: &str) -> Vec<CriteriaRule> {
    js_entries(json)
        .into_iter()
        .map(|(key, value)| {
            let shown = js_json_stringify(&value);
            rule(
                format!("prettier:{key}"),
                format!("Prettier setting \"{key}\" = {shown}"),
                file,
                format!("{key} = {shown}"),
            )
        })
        .collect()
}

/// `content.split(/\r?\n/)`
fn lines(content: &str) -> impl Iterator<Item = &str> {
    content
        .split('\n')
        .map(|l| l.strip_suffix('\r').unwrap_or(l))
}

fn parse_editor_config(content: &str, file: &str) -> Vec<CriteriaRule> {
    let section_re = regex(r"^\[(.+)\]$", "");
    let kv_re = regex(r"^([\w.-]+)\s*=\s*(.+)$", "");
    let mut rules = Vec::new();
    let mut current = "*".to_owned();
    for raw in lines(content) {
        let line = js_trim(raw);
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }
        if let Some(m) = section_re.find(line).and_then(|m| m.group(1)) {
            line[m].clone_into(&mut current);
            continue;
        }
        if let Some(m) = kv_re.find(line)
            && let (Some(k), Some(v)) = (m.group(1), m.group(2))
        {
            let (key, value) = (&line[k], &line[v]);
            rules.push(rule(
                format!("editorconfig:{current}:{key}"),
                format!("EditorConfig \"{key}\" = \"{value}\" for {current}"),
                file,
                format!("[{current}] {key} = {value}"),
            ));
        }
    }
    rules
}

fn parse_flat_toml(content: &str, file: &str, prefix: &str) -> Vec<CriteriaRule> {
    let kv_re = regex(r"^([\w.-]+)\s*=\s*(.+)$", "");
    let mut rules = Vec::new();
    for raw in lines(content) {
        let line = js_trim(raw);
        if line.is_empty() || line.starts_with('#') || line.starts_with('[') {
            continue;
        }
        if let Some(m) = kv_re.find(line)
            && let (Some(k), Some(v)) = (m.group(1), m.group(2))
        {
            let (key, value) = (&line[k], &line[v]);
            rules.push(rule(
                format!("{prefix}:{key}"),
                format!("{prefix} \"{key}\" = {value}"),
                file,
                format!("{key} = {value}"),
            ));
        }
    }
    rules
}

fn extract_heading_rules(content: &str, file: &str) -> Vec<CriteriaRule> {
    let heading_re = regex(r"^(##|###)\s+(.+)$", "gm");
    let mut rules = Vec::new();
    let mut index = 0;
    for m in heading_re.find_iter(content) {
        let Some(range) = m.group(2) else { continue };
        let heading = js_trim(&content[range]);
        if !heading.is_empty() {
            index += 1;
            rules.push(rule(
                format!("{file}:{index}"),
                heading.to_owned(),
                file,
                format!("Section \"{heading}\""),
            ));
        }
    }
    rules
}

const RULE_SHAPED_DOCS: [&str; 6] = [
    "ARCHITECTURE.md",
    "CONTRIBUTING.md",
    "CLAUDE.md",
    "UX_CRITERIA.md",
    "UI_CRITERIA.md",
    "DX_CRITERIA.md",
];

/// A configuration file that is not JSON is skipped without a word, as in the TypeScript engine:
/// an `.eslintrc` in YAML contributes no rule and no warning (`docs/PARITY.md`).
fn try_parse_json(content: &str) -> Option<Value> {
    serde_json::from_str::<Value>(content)
        .ok()
        .filter(|v| matches!(v, Value::Object(_) | Value::Array(_)))
}

// Rules from a file are added only if it yielded any, and then the file counts as a source.
fn add(
    extracted: Vec<CriteriaRule>,
    file: &str,
    rules: &mut Vec<CriteriaRule>,
    sources: &mut BTreeSet<String>,
) {
    if !extracted.is_empty() {
        rules.extend(extracted);
        sources.insert(file.to_owned());
    }
}

/// # Errors
/// A listed file that cannot be read.
pub fn scan_criteria(
    root: &Path,
    root_entries: &[String],
) -> Result<ScanResult<CriteriaProfile>, ScanError> {
    let has = |name: &str| root_entries.iter().any(|e| e == name);
    let mut files_read: Vec<PathBuf> = Vec::new();
    let mut rules: Vec<CriteriaRule> = Vec::new();
    let mut sources: BTreeSet<String> = BTreeSet::new();
    let read = |file: &str, files_read: &mut Vec<PathBuf>| -> Result<String, ScanError> {
        let path = root.join(file);
        files_read.push(path.clone());
        read_text(&path)
    };
    if let Some(file) = [".eslintrc.json", ".eslintrc"].into_iter().find(|f| has(f))
        && let Some(json) = try_parse_json(&read(file, &mut files_read)?)
    {
        add(eslint_rules(&json, file), file, &mut rules, &mut sources);
    }
    if has("biome.json")
        && let Some(json) = try_parse_json(&read("biome.json", &mut files_read)?)
    {
        add(
            biome_rules(&json, "biome.json"),
            "biome.json",
            &mut rules,
            &mut sources,
        );
    }
    if let Some(file) = [".prettierrc", ".prettierrc.json"]
        .into_iter()
        .find(|f| has(f))
        && let Some(json) = try_parse_json(&read(file, &mut files_read)?)
    {
        add(prettier_rules(&json, file), file, &mut rules, &mut sources);
    }
    if has(".editorconfig") {
        let text = read(".editorconfig", &mut files_read)?;
        add(
            parse_editor_config(&text, ".editorconfig"),
            ".editorconfig",
            &mut rules,
            &mut sources,
        );
    }
    if has("rustfmt.toml") {
        let text = read("rustfmt.toml", &mut files_read)?;
        add(
            parse_flat_toml(&text, "rustfmt.toml", "rustfmt"),
            "rustfmt.toml",
            &mut rules,
            &mut sources,
        );
    }
    if has("detekt.yml") {
        files_read.push(root.join("detekt.yml"));
        rules.push(rule(
            "detekt:present".to_owned(),
            "detekt configuration present (contents not parsed — see repository-intelligence risks)".to_owned(),
            "detekt.yml",
            "detekt.yml present".to_owned(),
        ));
        sources.insert("detekt.yml".to_owned());
    }
    for file in RULE_SHAPED_DOCS {
        if has(file) {
            let text = read(file, &mut files_read)?;
            add(
                extract_heading_rules(&text, file),
                file,
                &mut rules,
                &mut sources,
            );
        }
    }
    let mut sources: Vec<String> = sources.into_iter().collect();
    sources.sort_by(|a, b| utf16_cmp(a, b));
    Ok(ScanResult {
        profile: CriteriaProfile { rules, sources },
        files_read,
        dirs_listed: Vec::new(),
    })
}
