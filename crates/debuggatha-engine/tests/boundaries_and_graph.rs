//! Module boundaries, import extraction, import resolution and the dependency
//! graph for JavaScript and TypeScript. There is no one to copy exactly (the TypeScript engine read
//! imports with a pattern), so the cases below are the specification; the corpus test compares with
//! the TypeScript output and lists every difference. A mutation of the code must fail one of them.
#![allow(clippy::expect_used, clippy::unwrap_used)]
// a failed expect is a failed test
#![allow(clippy::too_many_lines)]
// tables of cases
// `assert!(x.is_empty())` is what these cases mean; comparing with an empty array of a long type says less.
#![allow(clippy::assert_is_empty)]
// The table rows are `(Option<String>, ..)`; the helper only builds the `Some`.
#![allow(clippy::unnecessary_wraps)]

use debuggatha_engine::analysis::imports::{ImportKind, js_imports};
use debuggatha_engine::analysis::{
    Manifest, ModuleKind, Resolution, Resolver, Unresolved, Workspace, build_graph,
    infer_boundaries,
};

fn ws(files: &[(&str, &str)]) -> Workspace {
    Workspace::from_files(files.iter().map(|(p, c)| (*p, *c)))
}

fn ids(w: &Workspace) -> Vec<String> {
    infer_boundaries(w).list.into_iter().map(|b| b.id).collect()
}

fn edges(w: &Workspace) -> Vec<String> {
    let g = build_graph(w, &infer_boundaries(w));
    g.edges
        .iter()
        .map(|e| format!("{} > {} {:?}", e.from, e.to, e.kind))
        .collect()
}

// ---- boundaries ----------------------------------------------------------------------------

#[test]
fn a_monorepo_layout_is_read_before_any_guess() {
    let w = ws(&[
        ("packages/core/package.json", r#"{"name":"@x/core"}"#),
        ("packages/core/src/a.ts", ""),
        ("packages/ui/src/b.tsx", ""),
        ("apps/web/package.json", r#"{"name":"web"}"#),
        ("apps/web/main.ts", ""),
        ("src/domain/c.ts", ""),
        ("README.md", ""),
    ]);
    let b = infer_boundaries(&w).list;
    let summary: Vec<_> = b
        .iter()
        .map(|b| {
            (
                b.id.as_str(),
                b.kind,
                b.name.as_deref(),
                b.manifest,
                b.files.len(),
            )
        })
        .collect();
    assert_eq!(
        summary,
        vec![
            (
                "apps/web",
                ModuleKind::Workspace,
                Some("web"),
                Manifest::Npm,
                1
            ),
            (
                "packages/core",
                ModuleKind::Package,
                Some("@x/core"),
                Manifest::Npm,
                1
            ),
            ("packages/ui", ModuleKind::Package, None, Manifest::None, 1),
        ],
        "src/ is not a boundary when a workspace layout exists"
    );
}

#[test]
fn without_a_workspace_the_directories_of_src_are_boundaries_by_name() {
    use ModuleKind::{Adapter, Domain, Feature, Layer};
    let w = ws(&[
        ("src/features/a.ts", ""),
        ("src/modules/a.ts", ""),
        ("src/services/a.ts", ""),
        ("src/components/a.ts", ""),
        ("src/adapters/a.ts", ""),
        ("src/infra/a.ts", ""),
        ("src/clients/a.ts", ""),
        ("src/core/a.ts", ""),
        ("src/ui/a.ts", ""),
        ("src/layers/a.ts", ""),
        ("src/whatever/a.ts", ""),
        ("src/node_modules/a.ts", ""),
        ("src/.hidden/a.ts", ""),
        ("src/loose.ts", ""),
    ]);
    let kinds: Vec<_> = infer_boundaries(&w)
        .list
        .into_iter()
        .map(|b| (b.id, b.kind))
        .collect();
    let want = [
        ("src/adapters", Adapter),
        ("src/clients", Adapter),
        ("src/components", Feature),
        ("src/core", Layer),
        ("src/features", Feature),
        ("src/infra", Adapter),
        ("src/layers", Layer),
        ("src/modules", Feature),
        ("src/services", Feature),
        ("src/ui", Layer),
        ("src/whatever", Domain),
    ];
    assert_eq!(
        kinds,
        want.iter()
            .map(|(i, k)| ((*i).to_owned(), *k))
            .collect::<Vec<_>>()
    );
}

#[test]
fn a_repository_with_nothing_to_infer_has_no_boundaries() {
    assert!(ids(&ws(&[("main.ts", ""), ("lib/a.ts", "")])).is_empty());
    assert!(ids(&ws(&[])).is_empty());
}

#[test]
fn boundary_files_are_the_analyzed_sources_under_the_directory() {
    let w = ws(&[
        ("packages/a/x.ts", ""),
        ("packages/a/y.rs", ""),
        ("packages/a/z.vue", ""),
        ("packages/a/data.json", ""),
        ("packages/a/notes.md", ""),
        ("packages/ab/x.ts", ""),
    ]);
    let b = infer_boundaries(&w).list;
    assert_eq!(
        b[0].files,
        ["packages/a/x.ts", "packages/a/y.rs", "packages/a/z.vue"]
    );
    assert_eq!(b[1].files, ["packages/ab/x.ts"]);
}

#[test]
fn a_cargo_workspace_gives_a_boundary_per_member() {
    let w = ws(&[
        (
            "Cargo.toml",
            "[workspace]\nmembers = [\"crates/*\", \"tools/xtask\", \"vendored/*\"]\nexclude = [\"crates/skip\"]\n",
        ),
        ("crates/a/Cargo.toml", "[package]\nname = \"my-a\"\n"),
        ("crates/a/src/lib.rs", ""),
        ("crates/skip/Cargo.toml", "[package]\nname = \"skip\"\n"),
        ("crates/nomanifest/src/lib.rs", ""),
        ("tools/xtask/Cargo.toml", "[package]\nname = \"xtask\"\n"),
        ("vendored/v/Cargo.toml", "[package]\nname = \"v\"\n"),
    ]);
    let b = infer_boundaries(&w).list;
    let got: Vec<_> = b
        .iter()
        .map(|b| (b.id.as_str(), b.name.as_deref(), b.manifest, b.kind))
        .collect();
    assert_eq!(
        got,
        vec![
            (
                "crates/a",
                Some("my-a"),
                Manifest::Cargo,
                ModuleKind::Package
            ),
            (
                "tools/xtask",
                Some("xtask"),
                Manifest::Cargo,
                ModuleKind::Package
            ),
            (
                "vendored/v",
                Some("v"),
                Manifest::Cargo,
                ModuleKind::Package
            ),
        ]
    );
}

#[test]
fn a_glob_the_inference_does_not_read_is_stated() {
    let w = ws(&[
        (
            "Cargo.toml",
            "[workspace]\nmembers = [\"a/**\", \"b?\", \"c/*/d/*\"]\n",
        ),
        ("a/x/Cargo.toml", "[package]\nname = \"x\"\n"),
    ]);
    let found = infer_boundaries(&w);
    assert!(found.list.is_empty());
    assert_eq!(found.limits.len(), 3);
    assert!(found.limits[0].contains("a/**"));
}

#[test]
fn a_root_package_beside_a_workspace_is_a_boundary_that_owns_the_rest() {
    let w = ws(&[
        (
            "Cargo.toml",
            "[package]\nname = \"root-crate\"\n[workspace]\nmembers = [\"crates/a\"]\n",
        ),
        ("src/lib.rs", ""),
        ("build.rs", ""),
        ("crates/a/Cargo.toml", "[package]\nname = \"a\"\n"),
        ("crates/a/src/lib.rs", ""),
    ]);
    let b = infer_boundaries(&w).list;
    assert_eq!(
        b.iter().map(|b| b.id.as_str()).collect::<Vec<_>>(),
        [".", "crates/a"]
    );
    assert_eq!(b[0].files, ["build.rs", "src/lib.rs"]);
    assert_eq!(b[0].name.as_deref(), Some("root-crate"));
    // A single package with no workspace falls back to the directories of src/.
    let single = ws(&[
        ("Cargo.toml", "[package]\nname = \"one\"\n"),
        ("src/a/mod.rs", ""),
    ]);
    assert_eq!(ids(&single), ["src/a"]);
}

#[test]
fn a_rust_module_file_belongs_to_its_directory() {
    let w = ws(&[
        ("src/a.rs", ""),
        ("src/a/b.rs", ""),
        ("src/c.rs", ""),
        ("src/b/x.rs", ""),
    ]);
    let b = infer_boundaries(&w).list;
    let a = b.iter().find(|b| b.id == "src/a").unwrap();
    assert_eq!(a.files, ["src/a.rs", "src/a/b.rs"]);
    assert!(b.iter().all(|b| !b.files.contains(&"src/c.rs".to_owned())));
}

// ---- imports -------------------------------------------------------------------------------

fn imports(path: &str, src: &str) -> Vec<(Option<String>, u32, ImportKind, bool)> {
    js_imports(path, src)
        .expect("parses")
        .into_iter()
        .map(|r| (r.specifier, r.line, r.kind, r.type_only))
        .collect()
}

fn s(x: &str) -> Option<String> {
    Some(x.to_owned())
}

#[test]
fn every_way_to_import_is_found() {
    use ImportKind::{Dynamic, ReExport, Require, Static, TypeQuery};
    let src = "\
import a from 'a';
import 'side-effect';
import type { T } from './types';
import { b,
  c } from \"b\";
export { d } from './d';
export * from './e';
export type { F } from './f';
const g = import('./g');
const h = require('h');
import i = require('i');
type J = import('./j').J;
const k = import(`./k`);
";
    assert_eq!(
        imports("a.ts", src),
        vec![
            (s("a"), 1, Static, false),
            (s("side-effect"), 2, Static, false),
            (s("./types"), 3, Static, true),
            (s("b"), 4, Static, false),
            (s("./d"), 6, ReExport, false),
            (s("./e"), 7, ReExport, false),
            (s("./f"), 8, ReExport, true),
            (s("./g"), 9, Dynamic, false),
            (s("h"), 10, Require, false),
            (s("i"), 11, Static, false),
            (s("./j"), 12, TypeQuery, true),
            (s("./k"), 13, Dynamic, false),
        ]
    );
}

#[test]
fn text_that_looks_like_an_import_is_not_one() {
    let src = "\
// import a from 'a';
/* import b from 'b'; */
const s = \"import c from 'c'\";
const t = `require('d')`;
const re = /from 'e'/;
const r = require('real');
const mine = { require: 1 }; mine.require('not');
function require2() {}
";
    assert_eq!(
        imports("a.js", src),
        vec![(s("real"), 6, ImportKind::Require, false)]
    );
}

#[test]
fn a_computed_specifier_is_kept_as_unknown() {
    let src = "\
const a = import(name);
const b = require(path + 'x');
const c = import(`./${lang}.js`);
const d = require();
const e = require('x', 'y');
";
    assert_eq!(
        imports("a.ts", src),
        vec![
            (None, 1, ImportKind::Dynamic, false),
            (None, 2, ImportKind::Require, false),
            (None, 3, ImportKind::Dynamic, false),
        ]
    );
}

#[test]
fn nested_calls_and_conditionals_are_found() {
    let src = "\
export async function f() {
  if (x) { const m = await import('./lazy'); }
  return [1].map(() => require('./cb'));
}
class C { m() { return import('./in-class'); } }
";
    let got: Vec<_> = imports("a.ts", src)
        .into_iter()
        .map(|r| (r.0, r.1))
        .collect();
    assert_eq!(
        got,
        vec![(s("./lazy"), 2), (s("./cb"), 3), (s("./in-class"), 5)]
    );
}

#[test]
fn imports_in_components_have_the_files_lines() {
    let vue = "<template>\n  <p>ñ</p>\n</template>\n<script setup lang=\"ts\">\nimport a from './a';\nconst b = import('./b');\n</script>\n";
    assert_eq!(
        imports("c.vue", vue),
        vec![
            (s("./a"), 5, ImportKind::Static, false),
            (s("./b"), 6, ImportKind::Dynamic, false),
        ]
    );
    let svelte =
        "<script>\n  import x from './x';\n</script>\n<style>\n a { color: red }\n</style>\n";
    assert_eq!(
        imports("c.svelte", svelte),
        vec![(s("./x"), 2, ImportKind::Static, false)]
    );
    let astro =
        "---\nimport L from '../L.astro';\n---\n<L />\n<script>\n  import './client';\n</script>\n";
    assert_eq!(
        imports("p.astro", astro),
        vec![
            (s("../L.astro"), 2, ImportKind::Static, false),
            (s("./client"), 6, ImportKind::Static, false),
        ]
    );
}

#[test]
fn a_file_that_cannot_be_read_is_none_and_other_languages_are_not_js() {
    assert!(js_imports("a.ts", "import a from ").is_none());
    assert!(js_imports("a.ts", "if (").is_none());
    assert!(js_imports("main.go", "import \"fmt\"").is_none());
    assert!(js_imports("a.rs", "use std::fmt;").is_none());
    // A file that merely ends early without a fatal error still gives what was read.
    assert_eq!(
        js_imports("a.ts", "import a from 'a';\n/* open")
            .unwrap()
            .len(),
        1
    );
}

// ---- resolution ----------------------------------------------------------------------------

fn resolve(files: &[(&str, &str)], from: &str, spec: &str) -> Resolution {
    let w = ws(files);
    let b = infer_boundaries(&w);
    Resolver::new(&w, &b.list).resolve(from, spec)
}

fn file(p: &str) -> Resolution {
    Resolution::File(p.to_owned())
}

#[test]
fn a_relative_path_finds_the_file_the_way_typescript_does() {
    let files = [
        ("src/a.ts", ""),
        ("src/x.ts", ""),
        ("src/x.js", ""),
        ("src/y.tsx", ""),
        ("src/z.d.ts", ""),
        ("src/dir/index.ts", ""),
        ("src/dir2/index.js", ""),
        ("src/only.json", ""),
        ("src/e.mts", ""),
        ("src/f.cts", ""),
        ("src/up.ts", ""),
        ("lib/l.ts", ""),
    ];
    let cases: &[(&str, &str, Resolution)] = &[
        ("src/a.ts", "./x", file("src/x.ts")),
        ("src/a.ts", "./y", file("src/y.tsx")),
        ("src/a.ts", "./z", file("src/z.d.ts")),
        ("src/a.ts", "./dir", file("src/dir/index.ts")),
        ("src/a.ts", "./dir2", file("src/dir2/index.js")),
        ("src/a.ts", "./only.json", file("src/only.json")),
        ("src/a.ts", "./x.js", file("src/x.ts")),
        ("src/a.ts", "./e.mjs", file("src/e.mts")),
        ("src/a.ts", "./f.cjs", file("src/f.cts")),
        ("src/dir/index.ts", "../up", file("src/up.ts")),
        ("src/dir/index.ts", "../../lib/l", file("lib/l.ts")),
        (
            "src/a.ts",
            ".",
            Resolution::Unresolved(Unresolved::NotFound),
        ),
        (
            "src/a.ts",
            "./missing",
            Resolution::Unresolved(Unresolved::NotFound),
        ),
        (
            "src/a.ts",
            "../../outside",
            Resolution::Unresolved(Unresolved::NotFound),
        ),
    ];
    for (from, spec, want) in cases {
        assert_eq!(&resolve(&files, from, spec), want, "{from} imports {spec}");
    }
}

#[test]
fn a_directory_with_a_manifest_entry_resolves_through_it() {
    let files = [
        ("src/lib/package.json", r#"{"main":"./dist-src/main.js"}"#),
        ("src/lib/dist-src/main.ts", ""),
        ("src/lib/index.ts", ""),
        (
            "src/typed/package.json",
            r#"{"types":"t.d.ts","main":"m.js"}"#,
        ),
        ("src/typed/t.d.ts", ""),
        ("src/typed/m.js", ""),
        ("src/a.ts", ""),
    ];
    assert_eq!(
        resolve(&files, "src/a.ts", "./lib"),
        file("src/lib/dist-src/main.ts")
    );
    assert_eq!(
        resolve(&files, "src/a.ts", "./typed"),
        file("src/typed/t.d.ts")
    );
}

#[test]
fn a_bundler_query_names_the_file_before_it() {
    let files = [("src/a.ts", ""), ("src/data.txt", ""), ("src/w.ts", "")];
    assert_eq!(
        resolve(&files, "src/a.ts", "./data.txt?url"),
        file("src/data.txt")
    );
    assert_eq!(
        resolve(&files, "src/a.ts", "./w?worker&inline"),
        file("src/w.ts")
    );
}

#[test]
fn tsconfig_paths_map_aliases_to_files() {
    let files = [
        (
            "tsconfig.json",
            r#"{
  // comments and trailing commas are legal here
  "compilerOptions": {
    "baseUrl": ".",
    "paths": {
      "@/*": ["src/*"],
      "@lib/*": ["lib/a/*", "lib/b/*"],
      "exact": ["src/exact-target.ts"],
    },
  },
}"#,
        ),
        ("src/a.ts", ""),
        ("src/shared/util.ts", ""),
        ("lib/b/thing.ts", ""),
        ("src/exact-target.ts", ""),
        ("top.ts", ""),
    ];
    let cases: &[(&str, Resolution)] = &[
        ("@/shared/util", file("src/shared/util.ts")),
        ("@lib/thing", file("lib/b/thing.ts")),
        ("exact", file("src/exact-target.ts")),
        ("top", file("top.ts")),
        (
            "@/nope",
            Resolution::Unresolved(Unresolved::AliasTargetNotFound),
        ),
        ("lodash", Resolution::External),
        ("node:fs", Resolution::External),
    ];
    for (spec, want) in cases {
        assert_eq!(&resolve(&files, "src/a.ts", spec), want, "{spec}");
    }
}

#[test]
fn the_nearest_config_wins_and_extends_is_followed() {
    let files = [
        (
            "tsconfig.base.json",
            r#"{"compilerOptions":{"baseUrl":".","paths":{"@base/*":["shared/*"]}}}"#,
        ),
        ("shared/s.ts", ""),
        (
            "packages/a/tsconfig.json",
            r#"{"extends":"../../tsconfig.base","compilerOptions":{}}"#,
        ),
        ("packages/a/src/i.ts", ""),
        (
            "packages/b/tsconfig.json",
            r#"{"compilerOptions":{"paths":{"@b/*":["local/*"]}}}"#,
        ),
        ("packages/b/local/l.ts", ""),
        ("packages/b/src/i.ts", ""),
        ("packages/c/src/i.ts", ""),
    ];
    // a: inherits the base paths (relative to the base's baseUrl).
    assert_eq!(
        resolve(&files, "packages/a/src/i.ts", "@base/s"),
        file("shared/s.ts")
    );
    // b: its own paths, relative to its own directory; the base's do not apply.
    assert_eq!(
        resolve(&files, "packages/b/src/i.ts", "@b/l"),
        file("packages/b/local/l.ts")
    );
    assert_eq!(
        resolve(&files, "packages/b/src/i.ts", "@base/s"),
        Resolution::External
    );
    // c: no config at all.
    assert_eq!(
        resolve(&files, "packages/c/src/i.ts", "@b/l"),
        Resolution::External
    );
}

#[test]
fn a_config_that_extends_itself_does_not_loop() {
    let files = [
        ("tsconfig.json", r#"{"extends":"./tsconfig.json"}"#),
        ("a.ts", ""),
        ("b.ts", ""),
    ];
    assert_eq!(resolve(&files, "a.ts", "./b"), file("b.ts"));
    assert_eq!(resolve(&files, "a.ts", "x"), Resolution::External);
}

#[test]
fn base_url_resolves_bare_specifiers_inside_the_repository() {
    let files = [
        ("tsconfig.json", r#"{"compilerOptions":{"baseUrl":"src"}}"#),
        ("src/app/a.ts", ""),
        ("src/shared/s.ts", ""),
    ];
    assert_eq!(
        resolve(&files, "src/app/a.ts", "shared/s"),
        file("src/shared/s.ts")
    );
    assert_eq!(
        resolve(&files, "src/app/a.ts", "react"),
        Resolution::External
    );
}

#[test]
fn a_package_of_the_workspace_is_named_by_its_package_name() {
    let files = [
        ("packages/core/package.json", r#"{"name":"@x/core"}"#),
        (
            "packages/core-extra/package.json",
            r#"{"name":"@x/core-extra"}"#,
        ),
        ("packages/app/package.json", r#"{"name":"app"}"#),
        ("packages/app/a.ts", ""),
    ];
    let cases: &[(&str, Resolution)] = &[
        ("@x/core", Resolution::Package("packages/core".into())),
        (
            "@x/core/sub/path",
            Resolution::Package("packages/core".into()),
        ),
        (
            "@x/core-extra",
            Resolution::Package("packages/core-extra".into()),
        ),
        (
            "@x/core-extra/y",
            Resolution::Package("packages/core-extra".into()),
        ),
        ("@x/corex", Resolution::External),
        ("@x/cor", Resolution::External),
        ("app", Resolution::Package("packages/app".into())),
    ];
    for (spec, want) in cases {
        assert_eq!(&resolve(&files, "packages/app/a.ts", spec), want, "{spec}");
    }
}

#[test]
fn a_mapping_that_misses_falls_through_to_the_workspace_package() {
    // TypeScript goes on to ordinary resolution when no target of a `paths` entry exists.
    let files = [
        ("packages/lib/package.json", r#"{"name":"lib"}"#),
        (
            "packages/app/tsconfig.json",
            r#"{"compilerOptions":{"paths":{"lib":["../nowhere/index.ts"],"@x/*":["../nowhere/*"]}}}"#,
        ),
        ("packages/app/a.ts", ""),
    ];
    assert_eq!(
        resolve(&files, "packages/app/a.ts", "lib"),
        Resolution::Package("packages/lib".into())
    );
    assert_eq!(
        resolve(&files, "packages/app/a.ts", "@x/y"),
        Resolution::Unresolved(Unresolved::AliasTargetNotFound)
    );
}

#[test]
fn names_no_package_can_have_are_unknown_aliases() {
    let files = [("src/a.ts", "")];
    for spec in ["#internal/x", "~/x", "$lib/x", "$app/state", "@/x"] {
        assert_eq!(
            resolve(&files, "src/a.ts", spec),
            Resolution::Unresolved(Unresolved::UnknownAlias),
            "{spec}"
        );
    }
    for spec in ["react", "@scope/pkg", "node:path", "bun:test", "lodash/fp"] {
        assert_eq!(
            resolve(&files, "src/a.ts", spec),
            Resolution::External,
            "{spec}"
        );
    }
}

// ---- the graph -----------------------------------------------------------------------------

#[test]
fn declared_and_import_edges_carry_their_evidence() {
    let w = ws(&[
        (
            "packages/app/package.json",
            r#"{"name":"app","dependencies":{"@x/core":"*","left-pad":"1"},"devDependencies":{"@x/test":"*"},"peerDependencies":{"@x/core":"*"}}"#,
        ),
        (
            "packages/app/a.ts",
            "import c from '@x/core';\nimport t from '@x/core/sub';\nimport o from '../other/o';\n",
        ),
        ("packages/app/b.ts", "import c from '@x/core';\n"),
        ("packages/core/package.json", r#"{"name":"@x/core"}"#),
        ("packages/core/c.ts", ""),
        ("packages/test/package.json", r#"{"name":"@x/test"}"#),
        ("packages/other/o.ts", ""),
    ]);
    let g = build_graph(&w, &infer_boundaries(&w));
    let shape: Vec<String> = g
        .edges
        .iter()
        .map(|e| {
            let ev: Vec<String> = e
                .evidence
                .iter()
                .map(|v| format!("{}:{}:{:?}", v.file, v.detail, v.line))
                .collect();
            format!("{} > {} {:?} [{}]", e.from, e.to, e.kind, ev.join(" | "))
        })
        .collect();
    assert_eq!(
        shape,
        vec![
            "packages/app > packages/core Declared [package.json:declares dependency on \"@x/core\":None]",
            "packages/app > packages/core Import [packages/app/a.ts:imports \"@x/core\":Some(1) | packages/app/a.ts:imports \"@x/core/sub\":Some(2) | packages/app/b.ts:imports \"@x/core\":Some(1)]",
            "packages/app > packages/other Import [packages/app/a.ts:imports \"../other/o\":Some(3)]",
            "packages/app > packages/test Declared [package.json:declares dependency on \"@x/test\":None]",
        ]
    );
    assert_eq!(
        g.nodes,
        [
            "packages/app",
            "packages/core",
            "packages/other",
            "packages/test"
        ]
    );
}

#[test]
fn an_import_inside_one_boundary_is_not_an_edge() {
    let w = ws(&[
        (
            "packages/a/x.ts",
            "import y from './y';\nimport z from '../a/z';\n",
        ),
        ("packages/a/y.ts", ""),
        ("packages/a/z.ts", ""),
    ]);
    assert!(edges(&w).is_empty());
}

#[test]
fn a_file_beside_a_directory_of_the_same_name_is_the_file() {
    // `./router` is `router.ts` when both exist; the directory is not what is imported.
    let w = ws(&[
        ("src/client/c.ts", "import r from '../router';\n"),
        ("src/router.ts", ""),
        ("src/router/trie.ts", ""),
        ("src/domain/d.ts", "import r from '../router/trie';\n"),
    ]);
    assert_eq!(edges(&w), ["src/domain > src/router Import"]);
}

#[test]
fn imports_that_resolve_to_nothing_are_listed_with_the_reason() {
    let w = ws(&[
        ("packages/a/package.json", r#"{"name":"a"}"#),
        (
            "packages/a/x.ts",
            "import m from './missing';\nimport k from '@/k';\nconst d = import(name);\nimport ok from 'react';\n",
        ),
    ]);
    let g = build_graph(&w, &infer_boundaries(&w));
    let got: Vec<_> = g
        .unresolved
        .iter()
        .map(|u| (u.line, u.specifier.as_deref(), u.reason))
        .collect();
    assert_eq!(
        got,
        vec![
            (1, Some("./missing"), "not-found"),
            (2, Some("@/k"), "unknown-alias"),
            (3, None, "computed"),
        ]
    );
    assert!(g.edges.is_empty());
}

#[test]
fn a_file_that_cannot_be_parsed_is_reported_not_silently_empty() {
    let w = ws(&[
        ("packages/a/ok.ts", "import x from '../b/x';\n"),
        ("packages/a/bad.ts", "import {{{{ from ;;;\n"),
        ("packages/b/x.ts", ""),
    ]);
    let g = build_graph(&w, &infer_boundaries(&w));
    assert_eq!(edges(&w), ["packages/a > packages/b Import"]);
    assert_eq!(g.unread_files, ["packages/a/bad.ts"]);
}

#[test]
fn type_only_imports_are_dependencies_too() {
    let w = ws(&[
        (
            "packages/a/x.ts",
            "import type { T } from '../b/t';\nexport type { U } from '../c/u';\n",
        ),
        ("packages/b/t.ts", ""),
        ("packages/c/u.ts", ""),
    ]);
    assert_eq!(
        edges(&w),
        [
            "packages/a > packages/b Import",
            "packages/a > packages/c Import"
        ]
    );
}

#[test]
fn declared_cargo_dependencies_are_edges_by_name_rename_and_path() {
    let w = ws(&[
        ("Cargo.toml", "[workspace]\nmembers = [\"crates/*\"]\n"),
        (
            "crates/app/Cargo.toml",
            "[package]\nname = \"app\"\n[dependencies]\ncore-lib = { path = \"../core\" }\nrenamed = { package = \"util-x\", version = \"1\" }\nserde = \"1\"\n[dev-dependencies]\nhelper = { path = \"../helper\" }\n[target.'cfg(unix)'.dependencies]\nunix-only = { path = \"../unix\" }\n",
        ),
        ("crates/core/Cargo.toml", "[package]\nname = \"core-lib\"\n"),
        ("crates/util/Cargo.toml", "[package]\nname = \"util-x\"\n"),
        ("crates/helper/Cargo.toml", "[package]\nname = \"helper\"\n"),
        ("crates/unix/Cargo.toml", "[package]\nname = \"unix\"\n"),
    ]);
    assert_eq!(
        edges(&w),
        [
            "crates/app > crates/core Declared",
            "crates/app > crates/helper Declared",
            "crates/app > crates/unix Declared",
            "crates/app > crates/util Declared",
        ]
    );
}

#[test]
fn paths_are_normalized_without_leaving_the_root_silently() {
    use debuggatha_engine::analysis::workspace::{dirname, join, normalize};
    assert_eq!(normalize("a/./b/../c"), "a/c");
    assert_eq!(normalize("./a"), "a");
    assert_eq!(normalize("a/../../b"), "../b");
    assert_eq!(normalize("../../x"), "../../x");
    assert_eq!(join("", "../x"), "../x");
    assert_eq!(join("a/b", "../c"), "a/c");
    assert_eq!(dirname("a/b/c.ts"), "a/b");
    assert_eq!(dirname("c.ts"), "");
}

#[test]
fn an_optional_call_of_require_is_not_an_import() {
    assert_eq!(
        imports(
            "a.js",
            "const a = require?.('x');\nconst b = obj.require('y');\n"
        ),
        vec![]
    );
}

#[test]
fn a_config_extending_a_package_does_not_read_a_directory_of_that_name() {
    // `"extends": "shared/base.json"` is a package path (node_modules), never a relative one.
    let files = [
        ("tsconfig.json", r#"{"extends":"shared/base.json"}"#),
        (
            "shared/base.json",
            r#"{"compilerOptions":{"baseUrl":".","paths":{"@s/*":["shared/*"]}}}"#,
        ),
        ("shared/s.ts", ""),
        ("a.ts", ""),
    ];
    assert_eq!(resolve(&files, "a.ts", "@s/s"), Resolution::External);
}

#[test]
fn a_target_inside_a_member_belongs_to_the_member_not_the_root_crate() {
    let w = ws(&[
        (
            "Cargo.toml",
            "[package]\nname = \"root\"\n[workspace]\nmembers = [\"crates/a\"]\n",
        ),
        ("src/lib.rs", "mod x;\n"),
        ("src/x.rs", ""),
        ("crates/a/Cargo.toml", "[package]\nname = \"a\"\n"),
        ("crates/a/src/lib.rs", "mod m;\nuse crate::m::f;\n"),
        ("crates/a/src/m.rs", "pub fn f() {}\n"),
    ]);
    assert!(edges(&w).is_empty(), "{:?}", edges(&w));
}

#[test]
fn a_path_dependency_on_a_directory_that_is_no_boundary_is_not_an_edge() {
    let w = ws(&[
        ("Cargo.toml", "[workspace]\nmembers = [\"crates/*\"]\n"),
        (
            "crates/app/Cargo.toml",
            "[package]\nname = \"app\"\n[dependencies]\nghost = { path = \"../ghost\" }\n",
        ),
    ]);
    assert!(edges(&w).is_empty());
}
