//! Reading a module's name (and, for Composer, its namespaces) out of a manifest.

use std::sync::OnceLock;

use super::xml;
use super::{Descriptor, ManifestFile};
use crate::analysis::structure::workspace::dirname;
use crate::syntax::{Language, Query, parse};

pub(super) fn read(id: &str, descriptor: &Descriptor, path: &str, text: &str) -> ManifestFile {
    let spec = descriptor.name.as_deref().unwrap_or_default();
    let name = match spec.split_once(':') {
        Some(("toml", key)) => toml_key(text, key),
        Some(("json", key)) => json_key(text, key),
        Some(("yaml", key)) => yaml_key(text, key),
        Some(("xml", element)) => xml_element(text, element),
        None if spec == "go-mod" => go_module(text),
        None if spec == "mix-app" => mix_app(path, text),
        None if spec == "dir-name" => dir_name(path),
        None if spec == "swift-package" => swift_package_name(path, text),
        _ => None,
    }
    .or_else(|| stem(path, &descriptor.manifest));
    ManifestFile {
        ecosystem: id.to_owned(),
        path: path.to_owned(),
        dir: dirname(path).to_owned(),
        name,
        namespaces: if id == "php" { psr4(text) } else { Vec::new() },
    }
}

/// A manifest named after its project (`App.csproj`) is called by the file name.
fn stem(path: &str, manifest: &str) -> Option<String> {
    let ext = manifest.strip_prefix('*')?;
    let file = path.rsplit('/').next()?;
    file.strip_suffix(ext).map(str::to_owned)
}

fn toml_key(text: &str, key: &str) -> Option<String> {
    let table: toml::Table = text.parse().ok()?;
    let mut parts = key.split('.');
    let mut value = table.get(parts.next()?)?;
    for part in parts {
        value = value.get(part)?;
    }
    value.as_str().map(str::to_owned)
}

fn json_key(text: &str, key: &str) -> Option<String> {
    let json: serde_json::Value = serde_json::from_str(text).ok()?;
    json.get(key)?.as_str().map(str::to_owned)
}

/// A top-level scalar of a YAML mapping (`name: app`); nested keys are not read.
fn yaml_key(text: &str, key: &str) -> Option<String> {
    text.lines()
        .filter(|l| !l.starts_with([' ', '\t', '#']))
        .find_map(|l| {
            let (k, v) = l.split_once(':')?;
            (k.trim() == key).then(|| v.trim().trim_matches(['"', '\'']).to_owned())
        })
        .filter(|v| !v.is_empty())
}

/// The first element called `element` that is not inside `parent`.
fn xml_element(text: &str, element: &str) -> Option<String> {
    xml::texts(text).into_iter().find_map(|(path, value)| {
        let mut parts = path.rsplit('/');
        (parts.next() == Some(element) && parts.next() != Some("parent")).then_some(value)
    })
}

fn go_module(text: &str) -> Option<String> {
    text.lines().find_map(|l| {
        let rest = l.trim().strip_prefix("module")?;
        rest.starts_with([' ', '\t'])
            .then(|| rest.trim().trim_matches('"').to_owned())
    })
}

/// `Package(name: "X")` in a `Package.swift`.
fn swift_package_name(path: &str, text: &str) -> Option<String> {
    let query = "(call_expression . (simple_identifier) @f (#eq? @f \"Package\")
        (call_suffix (value_arguments (value_argument
            name: (value_argument_label (simple_identifier) @l (#eq? @l \"name\"))
            value: (line_string_literal text: (line_str_text) @v)))))";
    matches(Language::Swift, query, path, text)
        .iter()
        .find_map(|m| capture(m, "v").map(str::to_owned))
}

/// `.package(path: "../X")` in a `Package.swift`: a package of this repository.
fn swift_packages(path: &str, text: &str) -> Vec<Dependency> {
    let query = "(call_expression
        (prefix_expression target: (simple_identifier) @m (#eq? @m \"package\"))
        (call_suffix (value_arguments (value_argument
            name: (value_argument_label (simple_identifier) @l (#eq? @l \"path\"))
            value: (line_string_literal text: (line_str_text) @v)))))";
    matches(Language::Swift, query, path, text)
        .iter()
        .filter_map(|m| capture(m, "v"))
        .map(|p| Dependency::Path(p.to_owned()))
        .collect()
}

/// A Gradle project is called by its directory unless its settings rename it.
fn dir_name(path: &str) -> Option<String> {
    dirname(path)
        .rsplit('/')
        .next()
        .filter(|n| !n.is_empty())
        .map(str::to_owned)
}

/// The captures of each match of a query over a manifest: (name, text) pairs.
fn matches(language: Language, query: &str, path: &str, text: &str) -> Vec<Vec<(String, String)>> {
    let Ok(query) = Query::new(language, query) else {
        return Vec::new();
    };
    let Some(syntax) = parse(std::path::Path::new(path), text) else {
        return Vec::new();
    };
    let Ok(found) = query.run(&syntax, text) else {
        return Vec::new();
    };
    found
        .iter()
        .map(|m| {
            m.captures
                .iter()
                .filter_map(|c| {
                    let t = text.get(c.span.start as usize..c.span.end as usize)?;
                    Some((c.name.clone(), t.to_owned()))
                })
                .collect()
        })
        .collect()
}

fn capture<'a>(found: &'a [(String, String)], name: &str) -> Option<&'a str> {
    found
        .iter()
        .find(|(n, _)| n == name)
        .map(|(_, t)| t.as_str())
}

/// The entries of `deps` in a `mix.exs`: `{:name, ...}`, and `path:` for a sibling application.
fn mix_deps(text: &str) -> Vec<Dependency> {
    const DEPS: &str = "(call target: (identifier) @f (#any-of? @f \"def\" \"defp\")
        (arguments (identifier) @n (#eq? @n \"deps\"))
        (do_block (list (tuple . (atom) @dep";
    let mut out: Vec<Dependency> =
        matches(Language::Elixir, &format!("{DEPS}))))"), "mix.exs", text)
            .iter()
            .filter_map(|m| capture(m, "dep"))
            .map(|n| Dependency::Name(n.trim_start_matches(':').to_owned()))
            .collect();
    let with_path = format!(
        "{DEPS} (keywords (pair key: (keyword) @k value: (string (quoted_content) @path)))))))"
    );
    for m in matches(Language::Elixir, &with_path, "mix.exs", text) {
        if let (Some(k), Some(path)) = (capture(&m, "k"), capture(&m, "path"))
            && k.trim().trim_end_matches(':') == "path"
        {
            out.push(Dependency::Path(path.to_owned()));
        }
    }
    out
}

/// The projects a Kotlin DSL build depends on, `project(":a:b")`, called by the last segment.
fn gradle_projects(path: &str, text: &str) -> Vec<Dependency> {
    let query = "(call_expression . (identifier) @f (#eq? @f \"project\")
        (value_arguments (value_argument (string_literal) @arg)))";
    matches(Language::Kotlin, query, path, text)
        .iter()
        .filter_map(|m| capture(m, "arg"))
        .filter_map(|a| {
            a.trim_matches('"')
                .rsplit(':')
                .next()
                .filter(|n| !n.is_empty())
                .map(|n| Dependency::Name(n.to_owned()))
        })
        .collect()
}

/// `app: :name` in a `mix.exs`, read from the parse and not from the text.
fn mix_app(path: &str, text: &str) -> Option<String> {
    static QUERY: OnceLock<Option<Query>> = OnceLock::new();
    let query = QUERY
        .get_or_init(|| {
            Query::new(
                Language::Elixir,
                "(pair key: (keyword) @key value: (atom) @value)",
            )
            .ok()
        })
        .as_ref()?;
    let syntax = parse(std::path::Path::new(path), text)?;
    query.run(&syntax, text).ok()?.iter().find_map(|m| {
        let get = |name: &str| {
            let c = m.captures.iter().find(|c| c.name == name)?;
            text.get(c.span.start as usize..c.span.end as usize)
        };
        (get("key")?.trim().trim_end_matches(':') == "app")
            .then(|| get("value").map(|v| v.trim_start_matches(':').to_owned()))?
    })
}

fn psr4(text: &str) -> Vec<(String, String)> {
    let Ok(json) = serde_json::from_str::<serde_json::Value>(text) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for section in ["autoload", "autoload-dev"] {
        let Some(map) = json
            .get(section)
            .and_then(|a| a.get("psr-4"))
            .and_then(|p| p.as_object())
        else {
            continue;
        };
        for (namespace, dirs) in map {
            let dirs = match dirs {
                serde_json::Value::String(d) => vec![d.clone()],
                serde_json::Value::Array(a) => a
                    .iter()
                    .filter_map(|d| d.as_str().map(str::to_owned))
                    .collect(),
                _ => Vec::new(),
            };
            out.extend(dirs.into_iter().map(|d| (namespace.clone(), d)));
        }
    }
    out
}

/// What a manifest says it depends on: a name, or a path to another manifest's directory or file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Dependency {
    /// A package, by the name it is published under.
    Name(String),
    /// A package, by the path to its directory or manifest.
    Path(String),
}

pub(super) fn dependencies(spec: &str, path: &str, text: &str) -> Vec<Dependency> {
    let names = |v: Vec<String>| v.into_iter().map(Dependency::Name).collect();
    match spec.split_once(':') {
        Some(("json", key)) => names(json_keys(text, key)),
        Some(("yaml", key)) => yaml_section(text, key),
        Some(("toml-list", key)) => names(toml_list(text, key)),
        Some(("xml", path)) => names(
            xml::texts(text)
                .into_iter()
                .filter(|(p, _)| p.ends_with(path))
                .map(|(_, v)| v)
                .collect(),
        ),
        None if spec == "go-mod" => names(go_requires(text)),
        None if spec == "mix-deps" => mix_deps(text),
        None if spec == "swift-packages" => swift_packages(path, text),
        None if spec == "gradle-projects" => gradle_projects(path, text),
        None if spec == "msbuild-project-reference" => {
            xml::attributes(text, "ProjectReference", "Include")
                .into_iter()
                .map(|p| Dependency::Path(p.replace('\\', "/")))
                .collect()
        }
        _ => Vec::new(),
    }
}

fn json_keys(text: &str, key: &str) -> Vec<String> {
    let Ok(json) = serde_json::from_str::<serde_json::Value>(text) else {
        return Vec::new();
    };
    [key.to_owned(), format!("{key}-dev")]
        .iter()
        .filter_map(|k| json.get(k)?.as_object())
        .flat_map(|o| o.keys().cloned())
        .collect()
}

/// The entries of a top-level YAML mapping: `name:` alone is a name, `name:` with a nested `path:`
/// is a path dependency.
fn yaml_section(text: &str, section: &str) -> Vec<Dependency> {
    let mut out = Vec::new();
    let mut inside = false;
    let mut current: Option<String> = None;
    for line in text.lines().filter(|l| !l.trim_start().starts_with('#')) {
        if !line.starts_with([' ', '\t']) {
            inside = line
                .split_once(':')
                .is_some_and(|(k, _)| k.trim() == section || k.trim() == format!("dev_{section}"));
            current = None;
            continue;
        }
        if !inside {
            continue;
        }
        let indent = line.len() - line.trim_start().len();
        let Some((key, value)) = line.trim().split_once(':') else {
            continue;
        };
        if indent <= 2 {
            current = Some(key.trim().to_owned());
            out.push(Dependency::Name(key.trim().to_owned()));
        } else if key.trim() == "path"
            && let Some(name) = &current
        {
            out.retain(|d| *d != Dependency::Name(name.clone()));
            out.push(Dependency::Path(
                value.trim().trim_matches(['"', '\'']).to_owned(),
            ));
        }
    }
    out
}

/// `["a>=1", "b[extra]; python_version<'3'"]` gives `a` and `b`.
fn toml_list(text: &str, key: &str) -> Vec<String> {
    let Ok(table) = text.parse::<toml::Table>() else {
        return Vec::new();
    };
    let mut parts = key.split('.');
    let Some(mut value) = parts.next().and_then(|p| table.get(p)) else {
        return Vec::new();
    };
    for part in parts {
        let Some(next) = value.get(part) else {
            return Vec::new();
        };
        value = next;
    }
    value
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|v| v.as_str())
        .map(|req| {
            req.split(|c: char| "<>=!~;[ (@".contains(c))
                .next()
                .unwrap_or_default()
                .to_owned()
        })
        .collect()
}

/// The module paths of `require` and `replace ... =>` in a `go.mod`, in a block or on a line.
fn go_requires(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut block = false;
    for line in text
        .lines()
        .map(|l| l.split("//").next().unwrap_or("").trim())
    {
        if block {
            if line == ")" {
                block = false;
            } else if let Some(name) = line.split_whitespace().next() {
                out.push(name.to_owned());
            }
        } else if let Some(rest) = line
            .strip_prefix("require")
            .or_else(|| line.strip_prefix("replace"))
        {
            let rest = rest.trim();
            if rest == "(" {
                block = true;
            } else if let Some(name) = rest.split_whitespace().next() {
                out.push(name.to_owned());
            }
        }
    }
    out
}
