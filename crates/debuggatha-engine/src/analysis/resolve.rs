//! Turns an import specifier into what it names: a file of the repository, a workspace package,
//! something outside the repository, or nothing the analysis could find (with the reason).
//!
//! Order, as TypeScript and bundlers do it: a relative path; `tsconfig`/`jsconfig` `paths` (the
//! nearest config, followed through `extends`); `baseUrl`; the repository's own packages by name;
//! then everything else is outside the repository. Not read: `package.json` `exports` and
//! `imports`, bundler aliases (`vite.config`), `node_modules` (never scanned).

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use super::boundaries::{Manifest, ModuleBoundary};
use super::jsonc;
use super::workspace::{Workspace, dirname, join, normalize};

/// Why a specifier could not be resolved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unresolved {
    /// A relative path to a file that does not exist.
    NotFound,
    /// A `paths` alias matched, and no file exists at any of its targets.
    AliasTargetNotFound,
    /// A name no package can have (`@/x`, `~/x`, `#x`, `$lib/x`): an alias set elsewhere (a bundler,
    /// `package.json` `imports`) that the analysis does not read.
    UnknownAlias,
    /// `import(name)`, `require(name)`: the specifier is computed.
    Computed,
}

impl Unresolved {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NotFound => "not-found",
            Self::AliasTargetNotFound => "alias-target-not-found",
            Self::UnknownAlias => "unknown-alias",
            Self::Computed => "computed",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resolution {
    /// A file of the repository.
    File(String),
    /// A package of the workspace, by the id of its boundary.
    Package(String),
    /// Not in the repository: a dependency, a built-in.
    External,
    Unresolved(Unresolved),
}

/// `.js` and friends name the TypeScript file beside them under ESM.
const JS_TO_TS: &[(&str, &[&str])] = &[
    (".js", &[".ts", ".tsx"]),
    (".jsx", &[".tsx"]),
    (".mjs", &[".mts"]),
    (".cjs", &[".cts"]),
];

/// Appended to an extensionless specifier, in the order TypeScript tries them.
const EXTENSIONS: &[&str] = &[
    ".ts", ".tsx", ".d.ts", ".js", ".jsx", ".mts", ".cts", ".mjs", ".cjs", ".json",
];

/// What the `paths` of a config say about a specifier.
enum Mapped {
    /// No pattern matches.
    NoPattern,
    /// A pattern matches and none of its targets exists.
    Missed,
    File(String),
}

#[derive(Debug, Clone, Default)]
struct TsConfig {
    /// Where bare specifiers are tried (`baseUrl`), as a root-relative directory.
    base_url: Option<String>,
    /// `paths`: pattern and root-relative target templates, longest prefix first.
    paths: Vec<(String, Vec<String>)>,
}

pub struct Resolver<'w> {
    ws: &'w Workspace,
    /// Workspace packages by the name other code imports them with, longest name first.
    npm: Vec<(String, String)>,
    configs: RefCell<BTreeMap<String, Option<Rc<TsConfig>>>>,
}

impl<'w> Resolver<'w> {
    #[must_use]
    pub fn new(ws: &'w Workspace, boundaries: &[ModuleBoundary]) -> Self {
        let npm: Vec<(String, String)> = boundaries
            .iter()
            .filter(|b| b.manifest == Manifest::Npm)
            .filter_map(|b| Some((b.name.clone()?, b.id.clone())))
            .collect();
        Self {
            ws,
            npm,
            configs: RefCell::default(),
        }
    }

    /// What `specifier` names, written in `from`.
    #[must_use]
    pub fn resolve(&self, from: &str, specifier: &str) -> Resolution {
        // `./file.txt?url`, `./w.js?worker`: a bundler's query names the file before it.
        let specifier = specifier
            .split_once('?')
            .map_or(specifier, |(path, _)| path);
        if specifier == "."
            || specifier == ".."
            || specifier.starts_with("./")
            || specifier.starts_with("../")
        {
            return self.relative(from, specifier);
        }
        let mut alias_failed = false;
        if let Some(config) = self.config_for(from) {
            match self.through_paths(&config, specifier) {
                Mapped::File(file) => return Resolution::File(file),
                // TypeScript goes on to ordinary resolution when no target of a mapping exists.
                Mapped::Missed => alias_failed = true,
                Mapped::NoPattern => {}
            }
            if let Some(base) = &config.base_url
                && let Some(file) = self.probe(&join(base, specifier))
            {
                return Resolution::File(file);
            }
        }
        for (name, id) in &self.npm {
            if specifier == name
                || specifier
                    .strip_prefix(name.as_str())
                    .is_some_and(|r| r.starts_with('/'))
            {
                return Resolution::Package(id.clone());
            }
        }
        if alias_failed {
            return Resolution::Unresolved(Unresolved::AliasTargetNotFound);
        }
        if is_alias_shape(specifier) {
            return Resolution::Unresolved(Unresolved::UnknownAlias);
        }
        Resolution::External
    }

    fn relative(&self, from: &str, specifier: &str) -> Resolution {
        let target = join(dirname(from), specifier);
        match self.probe(&target) {
            Some(file) => Resolution::File(file),
            None => Resolution::Unresolved(Unresolved::NotFound),
        }
    }

    fn through_paths(&self, config: &TsConfig, specifier: &str) -> Mapped {
        let matched =
            config
                .paths
                .iter()
                .find_map(|(pattern, targets)| match pattern.split_once('*') {
                    Some((head, tail)) => (specifier.len() >= head.len() + tail.len()
                        && specifier.starts_with(head)
                        && specifier.ends_with(tail))
                    .then(|| {
                        (
                            targets,
                            Some(specifier[head.len()..specifier.len() - tail.len()].to_owned()),
                        )
                    }),
                    None => (pattern == specifier).then_some((targets, None)),
                });
        let Some((targets, captured)) = matched else {
            return Mapped::NoPattern;
        };
        for target in targets {
            let path = match &captured {
                Some(c) => target.replacen('*', c, 1),
                None => target.clone(),
            };
            if let Some(file) = self.probe(&normalize(&path)) {
                return Mapped::File(file);
            }
        }
        Mapped::Missed
    }

    /// The file `base` names: as written, with a TypeScript extension for a JavaScript one, with
    /// an extension added, or as a directory (`package.json` entry, then `index`).
    fn probe(&self, base: &str) -> Option<String> {
        for (js, ts) in JS_TO_TS {
            if let Some(stem) = base.strip_suffix(js) {
                for t in *ts {
                    let candidate = format!("{stem}{t}");
                    if self.ws.exists(&candidate) {
                        return Some(candidate);
                    }
                }
            }
        }
        if self.ws.exists(base) {
            return Some(base.to_owned());
        }
        for ext in EXTENSIONS {
            let candidate = format!("{base}{ext}");
            if self.ws.exists(&candidate) {
                return Some(candidate);
            }
        }
        if let Some(entry) = self.package_entry(base)
            && let Some(file) = self.probe_flat(&join(base, &entry))
        {
            return Some(file);
        }
        for ext in EXTENSIONS {
            let candidate = format!(
                "{}index{ext}",
                if base.is_empty() {
                    String::new()
                } else {
                    format!("{base}/")
                }
            );
            if self.ws.exists(&candidate) {
                return Some(candidate);
            }
        }
        None
    }

    /// Like `probe` without the directory step, so an entry cannot name itself in a loop.
    fn probe_flat(&self, base: &str) -> Option<String> {
        for (js, ts) in JS_TO_TS {
            if let Some(stem) = base.strip_suffix(js) {
                for t in *ts {
                    let candidate = format!("{stem}{t}");
                    if self.ws.exists(&candidate) {
                        return Some(candidate);
                    }
                }
            }
        }
        if self.ws.exists(base) {
            return Some(base.to_owned());
        }
        EXTENSIONS
            .iter()
            .map(|ext| format!("{base}{ext}"))
            .find(|c| self.ws.exists(c))
    }

    /// `main`, `module` or `types` of the `package.json` in `dir`.
    fn package_entry(&self, dir: &str) -> Option<String> {
        let text = self.ws.read(&format!("{dir}/package.json"))?;
        let json: serde_json::Value = serde_json::from_str(text).ok()?;
        ["types", "module", "main"]
            .iter()
            .find_map(|k| json.get(k)?.as_str().map(str::to_owned))
    }

    /// The nearest `tsconfig.json` (or `jsconfig.json`) at or above the file's directory.
    fn config_for(&self, from: &str) -> Option<Rc<TsConfig>> {
        let mut dir = dirname(from).to_owned();
        loop {
            for name in ["tsconfig.json", "jsconfig.json"] {
                let path = if dir.is_empty() {
                    name.to_owned()
                } else {
                    format!("{dir}/{name}")
                };
                if self.ws.exists(&path) {
                    return self.load_config(&path);
                }
            }
            if dir.is_empty() {
                return None;
            }
            dir = dirname(&dir).to_owned();
        }
    }

    fn load_config(&self, path: &str) -> Option<Rc<TsConfig>> {
        if let Some(cached) = self.configs.borrow().get(path) {
            return cached.clone();
        }
        // Guard against a cycle of `extends` before computing.
        self.configs.borrow_mut().insert(path.to_owned(), None);
        let loaded = self.compute_config(path).map(Rc::new);
        self.configs
            .borrow_mut()
            .insert(path.to_owned(), loaded.clone());
        loaded
    }

    fn compute_config(&self, path: &str) -> Option<TsConfig> {
        let json = jsonc::parse(self.ws.read(path)?)?;
        let dir = dirname(path);
        let mut config = TsConfig::default();
        let extends: Vec<&str> = match json.get("extends") {
            Some(serde_json::Value::String(s)) => vec![s.as_str()],
            Some(serde_json::Value::Array(a)) => a.iter().filter_map(|v| v.as_str()).collect(),
            _ => Vec::new(),
        };
        for parent in extends {
            // Only files of the repository: a config from a package is in `node_modules`.
            if !(parent.starts_with("./") || parent.starts_with("../")) {
                continue;
            }
            let mut target = join(dir, parent);
            if !self.ws.exists(&target) {
                target.push_str(".json");
            }
            if self.ws.exists(&target)
                && let Some(base) = self.load_config(&target)
            {
                config = (*base).clone();
            }
        }
        let options = json.get("compilerOptions");
        let base_url = options
            .and_then(|o| o.get("baseUrl"))
            .and_then(|b| b.as_str())
            .map(|b| join(dir, b));
        if let Some(b) = &base_url {
            config.base_url = Some(b.clone());
        }
        if let Some(paths) = options
            .and_then(|o| o.get("paths"))
            .and_then(|p| p.as_object())
        {
            // Targets are relative to `baseUrl` when the config sets one, else to the config.
            let anchor = base_url
                .or_else(|| config.base_url.clone())
                .unwrap_or_else(|| dir.to_owned());
            config.paths = paths
                .iter()
                .map(|(pattern, targets)| {
                    let list = targets
                        .as_array()
                        .map(|a| {
                            a.iter()
                                .filter_map(|t| t.as_str())
                                .map(|t| join(&anchor, t))
                                .collect()
                        })
                        .unwrap_or_default();
                    (pattern.clone(), list)
                })
                .collect();
            // Longest prefix wins, as in TypeScript.
            config
                .paths
                .sort_by_key(|(p, _)| std::cmp::Reverse(p.find('*').unwrap_or(p.len())));
        }
        Some(config)
    }
}

/// A specifier shape no package name has: an alias configured somewhere the analysis does not read.
fn is_alias_shape(specifier: &str) -> bool {
    specifier.starts_with(['#', '~', '$']) || specifier.starts_with("@/")
}
