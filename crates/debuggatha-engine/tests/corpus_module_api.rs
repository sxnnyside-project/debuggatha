//! Ownership against the TypeScript engine on the real corpus: ownership, public API, dead code and
//! change impact. Opt-in, like `corpus_graph`:
//! `DEBUGGATHA_CORPUS_DIR=<dir> DEBUGGATHA_TS_ANALYSIS=<json from scripts/parity/dump-analysis.ts> cargo test -p debuggatha-engine --test corpus_module_api --release -- --ignored --nocapture`
//!
//! Every difference is given a reason by a rule, and a difference no rule explains fails the test.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::print_stdout)] // a diagnostic test
#![allow(clippy::too_many_lines)] // one comparison, read top to bottom

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use debuggatha_engine::analysis::{
    DeadCodeKind, ExportForm, ExportGraph, PublicApiSurface, PublicApiSymbol, Resolver, Workspace,
    build_graph, build_public_api, detect_dead_code, detect_ownership, estimate_change_impact,
    infer_boundaries, loaded_by_convention, read_all,
};
use serde_json::Value;

fn strings(v: &Value) -> BTreeSet<String> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|s| s.as_str().unwrap().to_owned())
        .collect()
}

/// The pattern the TypeScript engine read exports with, one line at a time: `export` and one of
/// the declaration keywords and a name, or `export { .. } from` on one line.
fn ts_pattern_sees(line: &str, name: &str) -> bool {
    let Some(rest) = line.strip_prefix("export") else {
        return false;
    };
    if rest.trim_start().starts_with('{') {
        return rest.contains('}') && rest.contains("from") && rest.contains(name);
    }
    let mut words = rest.split_whitespace().peekable();
    if words.peek() == Some(&"declare") {
        words.next();
    }
    if words.peek() == Some(&"async") {
        words.next();
    }
    let Some(kw) = words.next() else {
        return false;
    };
    if !["function", "const", "class", "interface", "type", "enum"].contains(&kw) {
        return false;
    }
    words
        .next()
        .is_some_and(|w| w.trim_start_matches('*').starts_with(name))
}

/// Differences in the entrypoints of a package, with the check that each extra one is named by
/// the package's manifest.
const MANIFEST_ENTRY_NOTE: &str = "named by `exports` or by a `main` that points into a build folder, which the TypeScript engine did not follow";

struct Tally {
    explained: BTreeMap<&'static str, usize>,
    unexplained: Vec<String>,
}

impl Tally {
    fn new() -> Self {
        Self {
            explained: BTreeMap::new(),
            unexplained: Vec::new(),
        }
    }
    fn because(&mut self, reason: &'static str) {
        *self.explained.entry(reason).or_default() += 1;
    }
    fn report(&self, title: &str, same: usize) {
        println!(
            "{title}: same {same}, explained {}, unexplained {}",
            self.explained.values().sum::<usize>(),
            self.unexplained.len()
        );
        for (reason, n) in &self.explained {
            println!("    {n:5}  {reason}");
        }
        for u in self.unexplained.iter().take(25) {
            println!("    UNEXPLAINED {u}");
        }
    }
}

#[test]
#[ignore = "needs the corpus clones: run it with its `just corpus-*` recipe"]
fn ownership_api_dead_code_and_impact_against_the_typescript_engine() {
    let (Some(dir), Some(golden)) = (
        std::env::var_os("DEBUGGATHA_CORPUS_DIR"),
        std::env::var_os("DEBUGGATHA_TS_ANALYSIS"),
    ) else {
        panic!(
            "DEBUGGATHA_CORPUS_DIR / DEBUGGATHA_TS_ANALYSIS must be set: run it with its `just corpus-*` recipe"
        );
    };
    let ts: Value = serde_json::from_str(&std::fs::read_to_string(golden).unwrap()).unwrap();
    let (mut own_same, mut own_diff) = (0, Vec::new());
    let (mut entries_same, mut entries) = (0, Tally::new());
    let (mut exported_same, mut exported) = (0, Tally::new());
    let (mut exposed_same, mut exposed) = (0, Tally::new());
    let (mut dead_same, mut dead_old, mut dead_new) = (0, Tally::new(), Tally::new());
    let (mut imp_same, mut imp) = (0, Tally::new());
    let mut mine_dead_total = 0;
    let mut wide = 0usize;

    for (repo, expected) in ts.as_object().unwrap() {
        let root = Path::new(&dir).join(repo);
        let ws = Workspace::from_dir(&root);
        let found = infer_boundaries(&ws);
        let graph = build_graph(&ws, &found);
        let resolver = Resolver::new(&ws, &found.list);
        let facts = read_all(&ws);
        let exports = ExportGraph {
            facts: &facts,
            resolver: &resolver,
        };
        let apis = build_public_api(&ws, &found.list, &facts, &exports, &resolver);
        let dead = detect_dead_code(&ws, &found.list, &facts, &exports, &resolver, &apis);
        let by_module: BTreeMap<&str, &PublicApiSurface> =
            apis.iter().map(|a| (a.module_id.as_str(), a)).collect();
        let line_of = |file: &str, line: u32| -> String {
            std::fs::read_to_string(root.join(file))
                .ok()
                .and_then(|t| t.lines().nth(line as usize - 1).map(str::to_owned))
                .unwrap_or_default()
        };

        // Ownership: equal.
        let mine = detect_ownership(&ws, &found.list);
        for want in expected["ownership"].as_array().unwrap() {
            let id = want["moduleId"].as_str().unwrap();
            let Some(m) = mine.iter().find(|m| m.module_id == id) else {
                continue;
            };
            let w: Vec<&str> = want["owners"]
                .as_array()
                .unwrap()
                .iter()
                .map(|o| o.as_str().unwrap())
                .collect();
            // The TypeScript engine saw no file in a module of another language, so it named no owner.
            let unseen = w.is_empty()
                && found.list.iter().find(|b| b.id == id).is_some_and(|b| {
                    b.files.iter().all(|f| {
                        !["ts", "tsx", "js", "jsx", "mjs", "cjs"]
                            .contains(&f.rsplit('.').next().unwrap_or(""))
                    })
                });
            if unseen || w == m.owners.iter().map(String::as_str).collect::<Vec<_>>() {
                own_same += 1;
            } else {
                own_diff.push(format!("{repo} {id}: ts {w:?} rs {:?}", m.owners));
            }
        }

        // Public API.
        let mut extra_entries: BTreeSet<String> = BTreeSet::new();
        for want in expected["publicApi"].as_array().unwrap() {
            let id = want["moduleId"].as_str().unwrap();
            let Some(m) = by_module.get(id) else { continue };
            if !m.covered || m.language != "javascript" {
                continue;
            }
            let ts_ep: Vec<&str> = want["entrypoint"].as_str().into_iter().collect();
            let rs_ep: Vec<&str> = m.entrypoints.iter().map(String::as_str).collect();
            if ts_ep == rs_ep {
                entries_same += 1;
            } else {
                // The nearest `package.json` at or above the module.
                let mut at = id.to_owned();
                let manifest = loop {
                    if let Ok(t) = std::fs::read_to_string(root.join(&at).join("package.json")) {
                        break t;
                    }
                    match at.rsplit_once('/') {
                        Some((up, _)) => at = up.to_owned(),
                        None if at.is_empty() => break String::new(),
                        None => at = String::new(),
                    }
                };
                for e in rs_ep.iter().filter(|e| !ts_ep.contains(e)) {
                    extra_entries.insert((*e).to_owned());
                    let rel = e.strip_prefix(&format!("{id}/")).unwrap_or(e);
                    let stem = rel.rsplit_once('.').map_or(rel, |(s, _)| s);
                    let stem = stem.strip_prefix("src/").unwrap_or(stem);
                    if manifest.contains(stem) {
                        entries.because(MANIFEST_ENTRY_NOTE);
                    } else {
                        entries
                            .unexplained
                            .push(format!("{repo} {id}: extra entrypoint {e}"));
                    }
                }
                if ts_ep.iter().any(|t| !rs_ep.contains(t)) {
                    entries
                        .unexplained
                        .push(format!("{repo} {id}: ts {ts_ep:?} rs {rs_ep:?}"));
                }
            }
            let key = |file: &str, name: &str| format!("{file}:{name}");

            // exported
            let w: BTreeSet<String> = want["exported"]
                .as_array()
                .unwrap()
                .iter()
                .map(|s| key(s["file"].as_str().unwrap(), s["name"].as_str().unwrap()))
                .collect();
            let r: BTreeMap<String, &PublicApiSymbol> = m
                .exported
                .iter()
                .map(|s| (key(&s.file, &s.name), s))
                .collect();
            exported_same += w.iter().filter(|k| r.contains_key(*k)).count();
            for k in w.iter().filter(|k| !r.contains_key(*k)) {
                if k.rsplit(':').next().is_some_and(|n| n.starts_with("type ")) {
                    exported.because("the TypeScript engine kept the `type` keyword of a multi-line specifier in the name");
                } else {
                    exported
                        .unexplained
                        .push(format!("{repo} ts-only exported {k}"));
                }
            }
            for (k, s) in r.iter().filter(|(k, _)| !w.contains(*k)) {
                match s.form {
                    ExportForm::Star => exported.because("`export * from` is followed (the TypeScript engine did not read it)"),
                    ExportForm::Default => exported.because("`export default` is a name (the TypeScript engine did not read it)"),
                    ExportForm::Specifier => exported.because("`export { a }` of a local is a name (the TypeScript engine did not read it)"),
                    ExportForm::Namespace => exported.because("`export * as ns from` is a name (the TypeScript engine did not read it)"),
                    ExportForm::Declaration | ExportForm::ReExport => {
                        let line = line_of(&s.file, s.line);
                        if extra_entries.contains(&s.file) {
                            exported.because("declared in an entrypoint only this engine found");
                        } else if line.contains(&format!("type {}", s.name)) {
                            exported.because("a `type X` specifier: the TypeScript engine kept the keyword in the name");
                        } else if !ts_pattern_sees(&line, &s.name) {
                            exported.because("a form the TypeScript line pattern does not match (`export type {`, `let`, `var`, `abstract`, a specifier over several lines, a leading BOM)");
                        } else {
                            exported.unexplained.push(format!("{repo} rs-only exported {k}: {line}"));
                        }
                    }
                }
            }

            // exposed
            let w: BTreeSet<String> = want["unexpectedlyExposed"]
                .as_array()
                .unwrap()
                .iter()
                .map(|s| key(s["file"].as_str().unwrap(), s["name"].as_str().unwrap()))
                .collect();
            let r: BTreeMap<String, &PublicApiSymbol> = m
                .unexpectedly_exposed
                .iter()
                .map(|s| (key(&s.file, &s.name), s))
                .collect();
            exposed_same += w.iter().filter(|k| r.contains_key(*k)).count();
            let ns_targets: BTreeSet<String> =
                m.entrypoints
                    .iter()
                    .filter_map(|e| facts.get(e).map(|f| (e, f)))
                    .flat_map(|(e, f)| {
                        f.exports.iter().filter_map(move |x| match &x.origin {
                            debuggatha_engine::analysis::exports::Origin::From {
                                source,
                                imported,
                            } if imported == "*" => Some((e.clone(), source.clone())),
                            _ => None,
                        })
                    })
                    .filter_map(|(e, source)| match resolver.resolve(&e, &source) {
                        debuggatha_engine::analysis::Resolution::File(f) => Some(f),
                        _ => None,
                    })
                    .collect();
            let exported_defs: BTreeSet<(String, String)> = m
                .exported
                .iter()
                .filter_map(|s| s.definition.clone())
                .collect();
            for k in w.iter().filter(|k| !r.contains_key(*k)) {
                let (file, name) = k.rsplit_once(':').unwrap();
                let own = facts
                    .get(file)
                    .and_then(|f| f.exports.iter().find(|e| e.name == name));
                if name.starts_with("type ") {
                    exposed.because("the TypeScript engine kept the `type` keyword of a multi-line specifier in the name");
                } else if extra_entries.contains(file) || m.entrypoints.iter().any(|e| e == file) {
                    exposed.because("the file is an entrypoint only this engine found");
                } else if loaded_by_convention(file) {
                    exposed.because("a file a tool loads by name (test, config, route): its exports are not an importer's");
                } else if own.is_none() {
                    exposed.because("not an export in the syntax tree (the pattern matched text in a comment, a string or a template)");
                } else if name == "default" {
                    exposed.because(
                        "a default export is how a tool loads a file, not a name an importer uses",
                    );
                } else if own
                    .is_some_and(|e| matches!(e.form, ExportForm::ReExport | ExportForm::Namespace))
                {
                    exposed.because(
                        "a re-export forwards another file's name; it declares nothing here",
                    );
                } else if ns_targets.contains(file) {
                    exposed
                        .because("published as a member of an `export * as ns` of an entrypoint");
                } else if exported_defs.contains(&(file.to_owned(), name.to_owned())) {
                    exposed.because(
                        "re-exported by an entrypoint, so it is part of the declared surface",
                    );
                } else {
                    exposed
                        .unexplained
                        .push(format!("{repo} ts-only exposed {k}"));
                }
            }
            for (k, s) in r.iter().filter(|(k, _)| !w.contains(*k)) {
                let line = line_of(&s.file, s.line);
                if matches!(s.form, ExportForm::Specifier) {
                    exposed.because(
                        "`export { a }` of a local (the TypeScript engine did not read it)",
                    );
                } else if !ts_pattern_sees(&line, &s.name) {
                    exposed.because("a form the TypeScript line pattern does not match");
                } else {
                    exposed
                        .unexplained
                        .push(format!("{repo} rs-only exposed {k}: {line}"));
                }
            }
        }

        // Dead code.
        mine_dead_total += dead.len();
        let namespaced: BTreeSet<String> = facts
            .iter()
            .flat_map(|(f, x)| {
                x.exports.iter().filter_map(move |e| match &e.origin {
                    debuggatha_engine::analysis::exports::Origin::From { source, imported }
                        if imported == "*" =>
                    {
                        Some((f.clone(), source.clone()))
                    }
                    _ => None,
                })
            })
            .filter_map(|(f, source)| match resolver.resolve(&f, &source) {
                debuggatha_engine::analysis::Resolution::File(t) => Some(t),
                _ => None,
            })
            .collect();
        let key = |file: &str, sym: &str| format!("{file}:{sym}");
        let mine_by: BTreeMap<String, Vec<DeadCodeKind>> = {
            let mut m: BTreeMap<String, Vec<DeadCodeKind>> = BTreeMap::new();
            for d in &dead {
                m.entry(key(&d.file, &d.symbol)).or_default().push(d.kind);
            }
            m
        };
        let kind_name = |k: DeadCodeKind| {
            serde_json::to_value(k)
                .unwrap()
                .as_str()
                .unwrap()
                .to_owned()
        };
        let mut ts_keys: BTreeSet<(String, String)> = BTreeSet::new();
        for d in expected["deadCode"].as_array().unwrap() {
            let (file, sym, kind) = (
                d["file"].as_str().unwrap(),
                d["symbol"].as_str().unwrap(),
                d["kind"].as_str().unwrap(),
            );
            ts_keys.insert((key(file, sym), kind.to_owned()));
            if mine_by
                .get(&key(file, sym))
                .is_some_and(|ks| ks.iter().any(|k| kind_name(*k) == kind))
            {
                dead_same += 1;
                continue;
            }
            let module = found
                .list
                .iter()
                .filter(|b| b.files.iter().any(|f| f == file))
                .map(|b| b.id.as_str())
                .next();
            let api = module.and_then(|m| by_module.get(m));
            let public = api.is_some_and(|a| {
                a.exported.iter().any(|s| {
                    s.definition
                        .as_ref()
                        .is_some_and(|(f, n)| f == file && n == sym)
                })
            });
            let own = facts
                .get(file)
                .and_then(|f| f.exports.iter().find(|e| e.name == sym));
            if sym.starts_with("type ") {
                dead_old.because("the TypeScript engine kept the `type` keyword of a multi-line specifier in the name");
            } else if sym.starts_with('$') && !mine_by.contains_key(&key(file, sym)) {
                dead_old.because("a name that starts with `$`: the word boundary `\\b` of the TypeScript pattern can never match before it, so it saw no reference at all");
            } else if mine_by.contains_key(&key(file, sym)) {
                dead_old.because("both report it, as another kind");
            } else if loaded_by_convention(file) {
                dead_old.because("a file a tool loads by name (test, config, route)");
            } else if own.is_none() {
                dead_old.because("not an export in the syntax tree");
            } else if sym == "default" {
                dead_old.because("a default export is how a tool loads a file");
            } else if own
                .is_some_and(|e| matches!(e.form, ExportForm::ReExport | ExportForm::Namespace))
                && !public
            {
                dead_old
                    .because("a re-export forwards another file's name; it declares nothing here");
            } else if ws
                .files_with(&[
                    "ts", "tsx", "js", "jsx", "mjs", "cjs", "mts", "cts", "vue", "svelte", "astro",
                ])
                .into_iter()
                .filter(|f| !found.list.iter().any(|b| b.files.iter().any(|x| x == f)))
                .any(|f| ws.read(f).is_some_and(|t| contains_word(t, sym)))
            {
                dead_old.because("used by a file outside every module (tests, scripts), which the TypeScript engine did not search");
            } else if ws
                .files_with(&["vue", "svelte", "astro"])
                .into_iter()
                .any(|f| ws.read(f).is_some_and(|t| contains_word(t, sym)))
            {
                dead_old.because(
                    "used by a single-file component, which the TypeScript engine did not read",
                );
            } else if namespaced.contains(file) {
                dead_old.because("published as `export * as ns`: its members are read through the namespace, which is not followed, so the namespace counts as a use of all of them");
            } else if own.is_some_and(|e| e.used_here) {
                dead_old.because("read inside its own file: not dead here (the export reaching nobody is the surface report's)");
            } else if public {
                wide += 1;
                dead_old.because("public, and an import that cannot be narrowed to names (a namespace passed on, a dynamic or deep import) counts as a use");
            } else {
                dead_old
                    .unexplained
                    .push(format!("{repo} ts-only dead {file}:{sym} [{kind}]"));
            }
        }
        for d in &dead {
            let k = (key(&d.file, &d.symbol), kind_name(d.kind));
            if ts_keys.contains(&k) {
                continue;
            }
            // A finding the TypeScript engine did not make: every one is checked against the
            // text. A name that no other file mentions is certain; a name another file mentions
            // is listed for a human read.
            let word_elsewhere = std::fs::read_dir(&root).is_ok() && {
                let mut hits = 0;
                for f in ws.files_with(&[
                    "ts", "tsx", "js", "jsx", "mjs", "cjs", "mts", "cts", "vue", "svelte", "astro",
                ]) {
                    if f == d.file {
                        continue;
                    }
                    if ws.read(f).is_some_and(|t| contains_word(t, &d.symbol)) {
                        hits += 1;
                    }
                }
                hits > 0
            };
            if ts_keys.iter().any(|(kk, _)| *kk == k.0) {
                dead_new.because("both report it, as another kind");
            } else if d.kind == DeadCodeKind::Unreferenced {
                dead_new.because("a declaration nothing in its file reads (the TypeScript engine looked only at exports)");
            } else if !word_elsewhere {
                dead_new
                    .because("no other file mentions the name: the pattern missed the export form");
            } else {
                dead_new.because("another file mentions the name; read below");
                println!(
                    "    CHECK {repo} {}:{} [{}]",
                    d.file,
                    d.symbol,
                    kind_name(d.kind)
                );
            }
        }

        // Impact of changing each module.
        for want in expected["impact"].as_array().unwrap() {
            let id = want["module"].as_str().unwrap();
            let Some(b) = found.list.iter().find(|b| b.id == id) else {
                continue;
            };
            let got = estimate_change_impact(&b.files, &found.list, &graph, &apis);
            let a = strings(&want["affected"]);
            let d = strings(&want["downstream"]);
            let ga: BTreeSet<String> = got.affected_modules.iter().cloned().collect();
            let gd: BTreeSet<String> = got.downstream_modules.iter().cloned().collect();
            if a == ga && d == gd {
                imp_same += 1;
            } else if a.is_subset(&ga) && d.is_subset(&gd) {
                // This engine reaches more: the module owns Rust or component files, or the graph
                // has an edge the TypeScript engine did not find (`boundaries_and_graph` lists them).
                imp.because(
                    "reaches more: Rust and component files are read, and the graph has more edges",
                );
            } else {
                imp.unexplained
                    .push(format!("{repo} {id}: ts {a:?}/{d:?} rs {ga:?}/{gd:?}"));
            }
        }
    }

    println!("ownership: same {own_same}, different {}", own_diff.len());
    for l in &own_diff {
        println!("    DIFFERENT {l}");
    }
    entries.report("entrypoints", entries_same);
    exported.report("exported names", exported_same);
    exposed.report("exposed names", exposed_same);
    dead_old.report("dead code only the TypeScript engine reports", dead_same);
    dead_new.report("dead code only this engine reports", 0);
    imp.report("change impact of each module", imp_same);
    println!(
        "this engine reports {mine_dead_total} dead-code findings; {wide} of the TypeScript ones sit behind a wide use"
    );

    assert!(own_diff.is_empty(), "ownership differs");
    assert!(entries.unexplained.is_empty(), "entrypoints");
    assert!(exported.unexplained.is_empty(), "exported names");
    assert!(exposed.unexplained.is_empty(), "exposed names");
    assert!(dead_old.unexplained.is_empty(), "dead code (ts only)");
    assert!(imp.unexplained.is_empty(), "impact");
}

/// Is `word` a whole word of `text`?
fn contains_word(text: &str, word: &str) -> bool {
    let is_id = |c: char| c.is_alphanumeric() || c == '_' || c == '$';
    text.match_indices(word).any(|(i, _)| {
        let before = text[..i].chars().next_back();
        let after = text[i + word.len()..].chars().next();
        !before.is_some_and(is_id) && !after.is_some_and(is_id)
    })
}

/// The Rust surface against rustdoc's list of a crate's public items (an oracle that is not this
/// code). Opt-in: `cargo doc --no-deps -p debuggatha-core -p debuggatha-engine`, then
/// `DEBUGGATHA_RUSTDOC_DIR=target/doc DEBUGGATHA_REPO_DIR=. cargo test ... rust_surface_against_rustdoc`.
#[test]
#[ignore = "needs the corpus clones: run it with its `just corpus-*` recipe"]
fn rust_surface_against_rustdoc() {
    let (Some(doc), Some(repo)) = (
        std::env::var_os("DEBUGGATHA_RUSTDOC_DIR"),
        std::env::var_os("DEBUGGATHA_REPO_DIR"),
    ) else {
        panic!(
            "DEBUGGATHA_RUSTDOC_DIR / DEBUGGATHA_REPO_DIR must be set: run it with its `just corpus-*` recipe"
        );
    };
    let ws = Workspace::from_dir(Path::new(&repo));
    let found = infer_boundaries(&ws);
    let resolver = Resolver::new(&ws, &found.list);
    let facts = read_all(&ws);
    let exports = ExportGraph {
        facts: &facts,
        resolver: &resolver,
    };
    let apis = build_public_api(&ws, &found.list, &facts, &exports, &resolver);
    for (module, krate) in [
        ("crates/debuggatha-core", "debuggatha_core"),
        ("crates/debuggatha-engine", "debuggatha_engine"),
        ("crates/debuggatha-cli", "debuggatha_cli"),
    ] {
        let Some(api) = apis.iter().find(|a| a.module_id == module) else {
            continue;
        };
        let Ok(html) = std::fs::read_to_string(Path::new(&doc).join(krate).join("all.html")) else {
            println!("{krate}: no rustdoc output (a binary crate has none)");
            assert!(api.exported.is_empty(), "{krate}");
            continue;
        };
        // `a/b/struct.X.html` is `a::b::X`.
        let mut theirs: BTreeSet<String> = BTreeSet::new();
        for part in html.split("<li><a href=\"").skip(1) {
            let href = part.split('"').next().unwrap();
            let Some((dir, file)) = href.rsplit_once('/').or(Some(("", href))) else {
                continue;
            };
            let file = file.trim_end_matches(".html");
            let Some((_, name)) = file.split_once('.') else {
                continue;
            };
            let mut p: Vec<&str> = dir.split('/').filter(|s| !s.is_empty()).collect();
            p.push(name);
            theirs.insert(p.join("::"));
        }
        // rustdoc lists an item once, where it is declared when that path is public and at its
        // re-export otherwise: an alias of a name declared on a public path is not another item.
        let declared: BTreeSet<&String> = api
            .exported
            .iter()
            .filter(|s| s.form == ExportForm::Declaration)
            .map(|s| &s.name)
            .collect();
        let mine: BTreeSet<String> = api
            .exported
            .iter()
            .filter(|s| {
                s.form == ExportForm::Declaration
                    || s.definition
                        .as_ref()
                        // A name of another crate is listed by rustdoc under that crate.
                        .is_some_and(|(_, q)| !declared.contains(q))
            })
            .map(|s| s.name.clone())
            .collect();
        let modules: BTreeSet<String> = mine
            .iter()
            .filter(|m| mine.iter().any(|o| o.starts_with(&format!("{m}::"))))
            .cloned()
            .collect();
        let missing: Vec<&String> = theirs.difference(&mine).collect();
        let extra: Vec<&String> = mine
            .difference(&theirs)
            .filter(|m| !modules.contains(*m))
            .collect();
        println!(
            "{krate}: rustdoc {} items, this engine {} names ({} modules); missing {}, extra {}",
            theirs.len(),
            mine.len(),
            modules.len(),
            missing.len(),
            extra.len()
        );
        for m in missing.iter().take(20) {
            println!("    MISSING {m}");
        }
        for m in extra.iter().take(20) {
            println!("    EXTRA {m}");
        }
        assert!(missing.is_empty() && extra.is_empty(), "{krate}");
    }
}
