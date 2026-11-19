//! `.debuggatha/config.json`: the settings a team wants every run of the repository to share.
//!
//! It is a file inside the repository, so anyone who can change the repository can change it. That
//! is why it can only make a review stricter or quieter, never more powerful: it cannot enable an
//! analyzer that runs project code or uses the network, and it cannot turn on the semantic pass
//! (which sends code to a model). Those stay with the person running the review, as flags.

use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde_json::Value;

use crate::report::ReportFormat;

pub(crate) const CONFIG_FILE: &str = ".debuggatha/config.json";
const SEVERITIES: [&str; 6] = ["informational", "low", "medium", "high", "critical", "none"];

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Raw {
    #[serde(rename = "//")]
    _note: Option<String>,
    review: Option<RawReview>,
    analyzers: Option<RawAnalyzers>,
    ignore: Option<Vec<String>>,
    overrides: Option<Vec<RawOverride>>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct RawReview {
    fail_on: Option<String>,
    format: Option<String>,
    packs: Option<Vec<String>>,
    default_policy: Option<Value>,
    default_depth: Option<Value>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct RawAnalyzers {
    mode: Option<String>,
    disable: Option<Vec<String>>,
    timeout_seconds: Option<u64>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct RawOverride {
    paths: Vec<String>,
    fail_on: String,
}

#[derive(Debug, Default)]
pub(crate) struct Config {
    pub(crate) fail_on: Option<String>,
    pub(crate) format: Option<ReportFormat>,
    pub(crate) packs: Vec<String>,
    pub(crate) analyzers_off: bool,
    pub(crate) disable: Vec<String>,
    pub(crate) timeout_seconds: Option<u64>,
    pub(crate) ignore: Vec<String>,
    pub(crate) overrides: Vec<(Vec<String>, String)>,
}

#[derive(Debug, Default)]
pub(crate) struct Loaded {
    pub(crate) config: Config,
    /// The directory `ignore` and `overrides` patterns are relative to: where `.debuggatha` is.
    pub(crate) root: Option<PathBuf>,
    pub(crate) warnings: Vec<String>,
}

/// The nearest config at or above `start`; a repository root ends the search.
fn find(start: &Path) -> Option<PathBuf> {
    let mut dir = start;
    loop {
        let candidate = dir.join(CONFIG_FILE);
        if candidate.is_file() {
            return Some(candidate);
        }
        if dir.join(".git").exists() {
            return None;
        }
        dir = dir.parent()?;
    }
}

fn refused(key: &str) -> Option<&'static str> {
    match key {
        "semantic" => Some(
            "the semantic pass sends code to a model, so only the person running the review can turn it on (--semantic)",
        ),
        "enable" => Some(
            "enabling an analyzer that runs project code or uses the network is the decision of the person running the review (--analyzer)",
        ),
        _ => None,
    }
}

/// Names a refused setting before the generic "unknown field" a strict parser would give.
fn refusal(value: &Value) -> Option<String> {
    let top = value.as_object()?;
    let hit = |object: &serde_json::Map<String, Value>, path: &str| {
        object.keys().find_map(|k| {
            refused(k).map(|why| format!("\"{path}{k}\" is not allowed in the file: {why}."))
        })
    };
    hit(top, "").or_else(|| hit(top.get("analyzers")?.as_object()?, "analyzers."))
}

fn one_of(what: &str, value: &str, allowed: &[&str]) -> Result<(), String> {
    if allowed.contains(&value) {
        Ok(())
    } else {
        Err(format!(
            "{what}: \"{value}\" is not one of {}.",
            allowed.join(", ")
        ))
    }
}

/// # Errors
/// The file is not valid JSON, names a setting that does not exist or is refused, or holds a value
/// outside what the setting takes. The message never carries the path of the machine.
pub(crate) fn load(start: &Path) -> Result<Loaded, String> {
    let Some(path) = find(start) else {
        return Ok(Loaded::default());
    };
    let text =
        std::fs::read_to_string(&path).map_err(|_| format!("{CONFIG_FILE} cannot be read."))?;
    let value: Value = serde_json::from_str(&text)
        .map_err(|e| format!("{CONFIG_FILE} is not valid JSON ({e})."))?;
    if let Some(why) = refusal(&value) {
        return Err(format!("{CONFIG_FILE} has a problem: {why}"));
    }
    let raw: Raw =
        serde_json::from_value(value).map_err(|e| format!("{CONFIG_FILE} has a problem: {e}."))?;
    let problem = |e: String| format!("{CONFIG_FILE} has a problem: {e}");

    let mut warnings = Vec::new();
    let mut config = Config::default();
    if let Some(review) = raw.review {
        if review.default_policy.is_some() || review.default_depth.is_some() {
            warnings.push(
                "review.defaultPolicy and review.defaultDepth were never read and are ignored; remove them."
                    .to_owned(),
            );
        }
        if let Some(fail_on) = &review.fail_on {
            one_of("review.failOn", fail_on, &SEVERITIES).map_err(problem)?;
        }
        config.fail_on = review.fail_on;
        if let Some(format) = review.format {
            config.format = Some(ReportFormat::parse(&format).ok_or_else(|| {
                problem(format!(
                    "review.format: \"{format}\" is not a report format."
                ))
            })?);
        }
        config.packs = review.packs.unwrap_or_default();
    }
    if let Some(analyzers) = raw.analyzers {
        if let Some(mode) = &analyzers.mode {
            one_of("analyzers.mode", mode, &["auto", "off"]).map_err(problem)?;
        }
        config.analyzers_off = analyzers.mode.as_deref() == Some("off");
        config.disable = analyzers.disable.unwrap_or_default();
        config.timeout_seconds = analyzers.timeout_seconds.filter(|s| *s > 0);
    }
    config.ignore = raw.ignore.unwrap_or_default();
    for o in raw.overrides.unwrap_or_default() {
        if o.paths.is_empty() {
            return Err(problem(
                "overrides.paths needs at least one path.".to_owned(),
            ));
        }
        one_of("overrides.failOn", &o.fail_on, &SEVERITIES).map_err(problem)?;
        config.overrides.push((o.paths, o.fail_on));
    }
    let root = path.parent().and_then(Path::parent).map(Path::to_path_buf);
    Ok(Loaded {
        config,
        root,
        warnings,
    })
}

/// A glob as a monorepo config writes it: `**` crosses directories, `*` and `?` stay inside one,
/// and a pattern with no slash matches that name at any depth (`*.generated.ts`). A directory
/// pattern covers what is inside it.
pub(crate) fn glob_matches(glob: &str, path: &str) -> bool {
    let glob = glob.strip_prefix("./").unwrap_or(glob);
    let pattern: Vec<char> = glob.chars().collect();
    let anywhere = !glob.contains('/');
    let chars: Vec<char> = path.chars().collect();
    // Every start a pattern may begin at, and every end it may stop at (a directory boundary).
    let starts = std::iter::once(0).chain(
        chars
            .iter()
            .enumerate()
            .filter(|(_, c)| **c == '/')
            .map(|(i, _)| i + 1),
    );
    let ends: Vec<usize> = chars
        .iter()
        .enumerate()
        .filter(|(_, c)| **c == '/')
        .map(|(i, _)| i)
        .chain(std::iter::once(chars.len()))
        .collect();
    let mut starts: Vec<usize> = starts.collect();
    if !anywhere {
        starts.truncate(1);
    }
    starts
        .iter()
        .any(|&s| ends.iter().any(|&e| e >= s && full(&pattern, &chars[s..e])))
}

fn full(pattern: &[char], text: &[char]) -> bool {
    match pattern {
        [] => text.is_empty(),
        ['*', '*', '/', rest @ ..] => {
            // zero or more directories
            full(rest, text)
                || text
                    .iter()
                    .enumerate()
                    .any(|(i, c)| *c == '/' && full(rest, &text[i + 1..]))
        }
        ['*', '*', rest @ ..] => (0..=text.len()).any(|i| full(rest, &text[i..])),
        ['*', rest @ ..] => {
            let reach = text.iter().position(|c| *c == '/').unwrap_or(text.len());
            (0..=reach).any(|i| full(rest, &text[i..]))
        }
        ['?', rest @ ..] => text
            .first()
            .is_some_and(|c| *c != '/' && full(rest, &text[1..])),
        [c, rest @ ..] => text.first() == Some(c) && full(rest, &text[1..]),
    }
}

#[cfg(test)]
mod tests {
    use super::glob_matches;

    #[test]
    fn globs_follow_the_monorepo_conventions() {
        assert!(glob_matches("packages/legacy", "packages/legacy/a/b.ts"));
        assert!(glob_matches("packages/*/dist", "packages/api/dist/x.js"));
        assert!(!glob_matches("packages/*/dist", "packages/a/b/dist/x.js"));
        assert!(glob_matches("**/*.generated.ts", "src/deep/x.generated.ts"));
        assert!(glob_matches("**/*.generated.ts", "x.generated.ts"));
        assert!(glob_matches("*.generated.ts", "src/deep/x.generated.ts"));
        assert!(glob_matches("./vendor/**", "vendor/a/b.js"));
        assert!(!glob_matches("vendor/**", "src/vendor.js"));
        assert!(glob_matches("a?c.js", "abc.js"));
        assert!(!glob_matches("a?c.js", "a/c.js"));
        assert!(!glob_matches("packages/legacy", "packages/legacy2/a.ts"));
    }
}
