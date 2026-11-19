//! The analysis stages against the TypeScript engine's, on the real corpus. Opt-in:
//! `DEBUGGATHA_CORPUS_DIR=<dir> DEBUGGATHA_TS_ANALYSIS=<json from scripts/parity/dump-analysis.ts> cargo test -p debuggatha-engine --test corpus_graph --release -- --ignored --nocapture`
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::print_stdout)] // a diagnostic test
#![allow(clippy::too_many_lines)] // one comparison, read top to bottom

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use debuggatha_engine::analysis::{
    Manifest, Workspace, build_graph, build_layer_model, detect_cycles, detect_layer_violations,
    infer_boundaries,
};
use serde_json::Value;

const TS_EXTENSIONS: &[&str] = &["ts", "tsx", "js", "jsx", "mjs", "cjs"];

fn ext(p: &str) -> &str {
    p.rsplit_once('.').map_or("", |(_, e)| e)
}

/// Differences with the TypeScript engine that are the point of the port, each with its reason.
/// `(repo, from, to)` of an edge only the TypeScript engine found.
const TS_ONLY_EDGES: &[(&str, &str, &str, &str)] = &[
    (
        "astro-preset",
        "src/scaffold",
        "src/styles",
        "the file named (`../styles/global.css`) does not exist; the pattern counted a path, not a file",
    ),
    (
        "hono",
        "src/client",
        "src/router",
        "`../router` is `src/router.ts`, a file beside the directory of the same name; the pattern took the directory",
    ),
    (
        "hono",
        "src/middleware",
        "src/request",
        "`../../request` is `src/request.ts`, the same case",
    ),
    (
        "hono",
        "src/validator",
        "src/request",
        "`../request` is `src/request.ts`, the same case",
    ),
    (
        "zod",
        "packages/tsc",
        "packages/zod",
        "the only `zod` imports of the package sit in comments and in strings of benchmark text",
    ),
];

/// `(repo, from, to)` of an edge only this engine found in a JavaScript/TypeScript file.
const RUST_ONLY_EDGES: &[(&str, &str, &str, &str)] = &[(
    "ontara",
    "src/features",
    "src/shared",
    "`@/shared/...` is a `tsconfig` `paths` alias, which the pattern could not follow",
)];

/// Layers the TypeScript engine assigned by suffix or by `/word` prefix and this one does not,
/// because the layer word is not a whole word of the name (`ClientApp`, `Webhooks.API`, `tui`).
const LAYER_WORD_DIFFS: &[(&str, &str)] = &[
    ("eShop", "src/ClientApp"),
    ("eShop", "src/HybridApp"),
    ("eShop", "src/WebApp"),
    ("eShop", "src/WebAppComponents"),
    ("eShop", "src/WebhookClient"),
    ("eShop", "src/Webhooks.API"),
    ("orph-cli", "src/tui"),
];

/// Upward dependencies that only appear because Rust files are read now (each verified).
const NEW_VIOLATIONS: &[(&str, &str, &str)] =
    &[("psychoquine-cli", "src/application", "src/infrastructure")];

/// Cycles that only appear because Rust files are read now (each verified).
const NEW_CYCLES: &[(&str, &str)] = &[("orph-cli", "src/cli,src/commands,src/tui")];

/// Boundaries only the TypeScript engine had: `(repo, id, reason)`.
const TS_ONLY_BOUNDARIES: &[(&str, &str, &str)] = &[(
    "pm2-eid-is-discord",
    "src/types",
    "holds only `.d.ts` files, which a review does not own, so no file is under it",
)];

/// Files the TypeScript engine did not read: Rust, components, `.mts`, and every language and
/// manifest of the other ecosystems.
fn only_components_or_rust(files: &[String]) -> bool {
    const NOT_READ: &[&str] = &[
        "rs", "vue", "svelte", "astro", "mts", "cts", "py", "go", "dart", "php", "ex", "exs",
        "java", "kt", "cs", "rb", "swift",
    ];
    const MANIFESTS: &[&str] = &[
        "Cargo.toml",
        "go.mod",
        "pyproject.toml",
        "pubspec.yaml",
        "composer.json",
        "mix.exs",
        "pom.xml",
        "build.gradle",
        "build.gradle.kts",
        "Package.swift",
        "Gemfile",
    ];
    files.iter().all(|f| {
        let name = f.rsplit('/').next().unwrap_or(f);
        MANIFESTS.contains(&name) || name.ends_with(".csproj") || NOT_READ.contains(&ext(f))
    })
}

/// A boundary found from the manifest of an ecosystem the TypeScript engine does not read.
fn of_another_ecosystem(manifest: Manifest) -> bool {
    !matches!(manifest, Manifest::Npm | Manifest::None)
}

#[test]
#[ignore = "needs the corpus clones: run it with its `just corpus-*` recipe"]
fn boundaries_and_graph_against_the_typescript_engine() {
    let (Some(dir), Some(golden)) = (
        std::env::var_os("DEBUGGATHA_CORPUS_DIR"),
        std::env::var_os("DEBUGGATHA_TS_ANALYSIS"),
    ) else {
        panic!(
            "DEBUGGATHA_CORPUS_DIR / DEBUGGATHA_TS_ANALYSIS must be set: run it with its `just corpus-*` recipe"
        );
    };
    let ts: Value = serde_json::from_str(&std::fs::read_to_string(golden).unwrap()).unwrap();
    let mut unexplained: Vec<String> = Vec::new();
    let mut explained: BTreeMap<&str, usize> = BTreeMap::new();
    let mut identical = 0;
    let mut identical_cycles = 0;
    let mut unresolved = 0;
    for (repo, expected) in ts.as_object().unwrap() {
        let ws = Workspace::from_dir(&Path::new(&dir).join(repo));
        let found = infer_boundaries(&ws);
        let graph = build_graph(&ws, &found);
        let has_cargo = found.list.iter().any(|b| b.manifest == Manifest::Cargo);

        // Boundaries: id, kind, name, and the JavaScript/TypeScript files they own.
        let want: BTreeMap<String, (String, Value, Vec<String>)> = expected["boundaries"]
            .as_array()
            .unwrap()
            .iter()
            .map(|b| {
                (
                    b["id"].as_str().unwrap().to_owned(),
                    (
                        b["kind"].as_str().unwrap().to_owned(),
                        b["name"].clone(),
                        b["files"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .map(|f| f.as_str().unwrap().to_owned())
                            .collect(),
                    ),
                )
            })
            .collect();
        for (id, w) in &want {
            match found.list.iter().find(|b| &b.id == id) {
                None => {
                    if TS_ONLY_BOUNDARIES
                        .iter()
                        .any(|(r, i, _)| r == repo && i == id)
                    {
                        *explained.entry("boundary with no owned file").or_default() += 1;
                    } else if has_cargo {
                        *explained
                            .entry("replaced by the crates of a Cargo workspace")
                            .or_default() += 1;
                    } else if id.starts_with("src/")
                        && found.list.iter().any(|b| of_another_ecosystem(b.manifest))
                    {
                        *explained
                            .entry("the `src/` directories are a fallback, not used once a manifest names the modules")
                            .or_default() += 1;
                    } else {
                        unexplained.push(format!("{repo}: boundary {id} only in TS"));
                    }
                }
                Some(b) => {
                    let kind = serde_json::to_value(b.kind)
                        .unwrap()
                        .as_str()
                        .unwrap()
                        .to_owned();
                    let files: Vec<String> = b
                        .files
                        .iter()
                        .filter(|f| TS_EXTENSIONS.contains(&ext(f)))
                        .cloned()
                        .collect();
                    let name = b.name.clone().map_or(Value::Null, Value::String);
                    if (kind != w.0 || files != w.2) && of_another_ecosystem(b.manifest) {
                        *explained
                            .entry("a module the manifest of another ecosystem names")
                            .or_default() += 1;
                    } else if kind != w.0 || files != w.2 {
                        unexplained.push(format!("{repo}: boundary {id} kind or files differ"));
                    } else if name != w.1 {
                        // The TypeScript engine read only `package.json` names.
                        if w.1.is_null() && of_another_ecosystem(b.manifest) {
                            *explained.entry("name from another manifest").or_default() += 1;
                        } else {
                            unexplained.push(format!("{repo}: boundary {id} name differs"));
                        }
                    }
                }
            }
        }
        for b in &found.list {
            if !want.contains_key(&b.id) {
                if b.manifest == Manifest::Cargo {
                    *explained.entry("a crate of a Cargo workspace").or_default() += 1;
                } else if of_another_ecosystem(b.manifest) {
                    *explained
                        .entry("a module of an ecosystem the TypeScript engine does not read")
                        .or_default() += 1;
                } else {
                    unexplained.push(format!("{repo}: boundary {} only in Rust", b.id));
                }
            }
        }

        // Edges: the set of (from, to, kind).
        let key = |e: &Value| {
            (
                e["from"].as_str().unwrap().to_owned(),
                e["to"].as_str().unwrap().to_owned(),
                e["kind"].as_str().unwrap().to_owned(),
            )
        };
        let want_edges: BTreeSet<_> = expected["edges"]
            .as_array()
            .unwrap()
            .iter()
            .map(key)
            .collect();
        let got_edges: BTreeMap<_, _> = graph
            .edges
            .iter()
            .map(|e| {
                (
                    (
                        e.from.clone(),
                        e.to.clone(),
                        serde_json::to_value(e.kind)
                            .unwrap()
                            .as_str()
                            .unwrap()
                            .to_owned(),
                    ),
                    e.evidence
                        .iter()
                        .map(|v| v.file.clone())
                        .collect::<Vec<_>>(),
                )
            })
            .collect();
        for e in &want_edges {
            if got_edges.contains_key(e) {
                identical += 1;
            } else if let Some((_, _, _, why)) = TS_ONLY_EDGES
                .iter()
                .find(|(r, f, t, _)| r == repo && *f == e.0 && *t == e.1)
            {
                let _ = why;
                *explained.entry("edge the pattern invented").or_default() += 1;
            } else {
                unexplained.push(format!(
                    "{repo}: edge {} -> {} ({}) only in TS",
                    e.0, e.1, e.2
                ));
            }
        }
        for (e, files) in &got_edges {
            if want_edges.contains(e) {
                continue;
            }
            if RUST_ONLY_EDGES
                .iter()
                .any(|(r, f, t, _)| r == repo && *f == e.0 && *t == e.1)
            {
                *explained
                    .entry("edge through a tsconfig alias")
                    .or_default() += 1;
            } else if only_components_or_rust(files) {
                *explained
                    .entry("edge from Rust, components or .mts files")
                    .or_default() += 1;
            } else {
                unexplained.push(format!(
                    "{repo}: edge {} -> {} ({}) only in Rust",
                    e.0, e.1, e.2
                ));
            }
        }
        // Layers: only boundaries both engines have.
        let model = build_layer_model(&found.list);
        for (id, layer) in expected["layers"].as_object().unwrap() {
            let Some(ours) = model.assignments.get(id).map(|l| l.as_str()) else {
                continue;
            };
            if Some(ours) != layer.as_str() {
                if LAYER_WORD_DIFFS.iter().any(|(r, i)| r == repo && i == id) {
                    *explained
                        .entry("layer matched by a whole word")
                        .or_default() += 1;
                } else {
                    unexplained.push(format!("{repo}: layer of {id}: TS {layer} Rust {ours}"));
                }
            }
        }
        // Violations: the TypeScript engine found none on the corpus.
        for v in detect_layer_violations(&graph, &model) {
            if NEW_VIOLATIONS
                .iter()
                .any(|(r, f, t)| r == repo && *f == v.from && *t == v.to)
            {
                *explained
                    .entry("layer violation in Rust files")
                    .or_default() += 1;
            } else {
                unexplained.push(format!("{repo}: violation {} -> {}", v.from, v.to));
            }
        }
        assert_eq!(expected["violations"].as_array().unwrap().len(), 0);
        // Cycles: every cycle of the TypeScript engine, exactly; and the ones only Rust files make.
        let cycles = detect_cycles(&graph);
        for c in expected["cycles"].as_array().unwrap() {
            let same = cycles.iter().any(|ours| {
                serde_json::to_value(ours).unwrap()["modules"] == c["modules"]
                    && serde_json::to_value(ours).unwrap()["shortest_cycle"] == c["shortestCycle"]
                    && serde_json::to_value(ours).unwrap()["impact"] == c["impact"]
            });
            if same {
                identical_cycles += 1;
            } else {
                unexplained.push(format!("{repo}: cycle {} not reproduced", c["modules"]));
            }
        }
        for c in &cycles {
            let known = expected["cycles"]
                .as_array()
                .unwrap()
                .iter()
                .any(|t| t["modules"] == serde_json::to_value(&c.modules).unwrap());
            if !known {
                if NEW_CYCLES
                    .iter()
                    .any(|(r, m)| r == repo && c.modules.join(",") == *m)
                {
                    *explained.entry("cycle through Rust paths").or_default() += 1;
                } else if c.modules.iter().all(|m| {
                    found
                        .list
                        .iter()
                        .find(|b| &b.id == m)
                        .is_some_and(|b| only_components_or_rust(&b.files))
                }) {
                    *explained
                        .entry(
                            "cycle among modules of languages the TypeScript engine does not read",
                        )
                        .or_default() += 1;
                } else {
                    unexplained.push(format!("{repo}: cycle {:?} only in Rust", c.modules));
                }
            }
        }
        unresolved += graph.unresolved.len();
        assert!(
            graph.unread_files.is_empty(),
            "{repo}: {:?}",
            graph.unread_files
        );
    }
    println!(
        "edges identical with the TypeScript engine: {identical}; {unresolved} imports left unresolved"
    );
    for (what, n) in &explained {
        println!("explained: {n} x {what}");
    }
    for u in &unexplained {
        println!("UNEXPLAINED {u}");
    }
    assert!(
        unexplained.is_empty(),
        "differences nobody explained:\n{}",
        unexplained.join("\n")
    );
    let listed = explained
        .get("edge the pattern invented")
        .copied()
        .unwrap_or(0);
    assert_eq!(
        listed,
        TS_ONLY_EDGES.len(),
        "an expected difference stopped happening"
    );
    let count = |what: &str| explained.get(what).copied().unwrap_or(0);
    assert_eq!(
        count("edge through a tsconfig alias"),
        RUST_ONLY_EDGES.len()
    );
    assert_eq!(
        count("layer matched by a whole word"),
        LAYER_WORD_DIFFS.len()
    );
    assert_eq!(count("layer violation in Rust files"), NEW_VIOLATIONS.len());
    assert_eq!(count("cycle through Rust paths"), NEW_CYCLES.len());
    assert_eq!(identical_cycles, 3, "the TypeScript cycles are reproduced");
}
