//! Ownership, what a module exports, dead code and change impact. The cases are
//! the specification; the corpus test compares with the TypeScript engine and explains every
//! difference.
#![allow(clippy::expect_used, clippy::unwrap_used)]
// a failed expect is a failed test
#![allow(clippy::too_many_lines)] // tables of cases
// `assert!(x.is_empty())` is what these cases mean.
#![allow(clippy::assert_is_empty)]

use debuggatha_engine::analysis::{
    Confidence, DeadCodeFinding, DeadCodeKind, ExportGraph, ImpactConfidence, PublicApiSurface,
    Resolver, Workspace,
    boundaries::{Manifest, ModuleBoundary, ModuleKind},
    build_graph, build_public_api, detect_dead_code, detect_ownership, estimate_change_impact,
    infer_boundaries, read_all, read_facts,
};

const PKG: &str = r#"{"name":"@x/lib","private":true,"main":"dist/index.js"}"#;

struct World {
    ws: Workspace,
    apis: Vec<PublicApiSurface>,
    dead: Vec<DeadCodeFinding>,
}

fn world(files: &[(&str, &str)]) -> World {
    let ws = Workspace::from_files(files.iter().map(|(p, c)| (*p, *c)));
    let found = infer_boundaries(&ws);
    let resolver = Resolver::new(&ws, &found.list);
    let facts = read_all(&ws);
    let exports = ExportGraph {
        facts: &facts,
        resolver: &resolver,
    };
    let apis = build_public_api(&ws, &found.list, &facts, &exports, &resolver);
    let dead = detect_dead_code(&ws, &found.list, &facts, &exports, &resolver, &apis);
    World { ws, apis, dead }
}

fn lib(files: &[(&str, &str)]) -> World {
    let mut all = vec![("packages/lib/package.json", PKG)];
    all.extend_from_slice(files);
    world(&all)
}

fn names(w: &World) -> Vec<String> {
    let api = w
        .apis
        .iter()
        .find(|a| a.module_id == "packages/lib")
        .unwrap();
    api.exported.iter().map(|s| s.name.clone()).collect()
}

fn dead(w: &World, kind: DeadCodeKind) -> Vec<String> {
    w.dead
        .iter()
        .filter(|d| d.kind == kind)
        .map(|d| d.symbol.clone())
        .collect()
}

// ─── what a file exports ────────────────────────────────────────────────────────────────────

#[test]
fn every_form_of_export_is_a_name() {
    let src = "\
export function f() {}
export async function g() {}
export const { a, b: c } = obj;
export let [d, e] = arr;
export class K {}
export abstract class Abs {}
export interface I {}
export type T = 1;
export enum E { A }
export namespace N {}
export declare const decl: number;
const local = 1;
export { local, local as renamed };
export default function () {}
";
    let facts = read_facts("a.ts", src).unwrap();
    let mut got: Vec<&str> = facts.exports.iter().map(|e| e.name.as_str()).collect();
    got.sort_unstable();
    assert_eq!(
        got,
        [
            "Abs", "E", "I", "K", "N", "T", "a", "c", "d", "decl", "default", "e", "f", "g",
            "local", "renamed"
        ]
    );
}

#[test]
fn an_export_in_a_comment_or_a_string_is_not_one() {
    let src = "// export function nope() {}\nconst s = 'export const nope2 = 1';\n/* export { nope3 } from './x' */\nexport const real = 1;\n";
    let facts = read_facts("a.ts", src).unwrap();
    assert_eq!(facts.exports.len(), 1);
    assert_eq!(facts.exports[0].name, "real");
}

#[test]
fn an_export_that_spans_lines_reports_its_first_line() {
    let src = "export {\n  a,\n  b as c,\n} from './m';\n";
    let facts = read_facts("a.ts", src).unwrap();
    let c = facts.exports.iter().find(|e| e.name == "c").unwrap();
    assert_eq!(c.line, 3);
}

#[test]
fn a_file_that_does_not_parse_has_no_facts() {
    assert!(read_facts("a.ts", "export const = ;").is_none());
}

// ─── public API ─────────────────────────────────────────────────────────────────────────────

#[test]
fn the_entrypoint_of_a_built_package_is_its_source() {
    let w = lib(&[
        ("packages/lib/src/index.ts", "export const api = 1;\n"),
        ("packages/lib/src/inner.ts", "export const hidden = 1;\n"),
    ]);
    assert_eq!(names(&w), ["api"]);
    let a = &w.apis[0];
    assert_eq!(a.entrypoints, ["packages/lib/src/index.ts"]);
    assert_eq!(a.unexpectedly_exposed.len(), 1);
    assert_eq!(a.unexpectedly_exposed[0].name, "hidden");
}

#[test]
fn exports_of_the_manifest_are_every_entrypoint() {
    let w = world(&[
        (
            "packages/lib/package.json",
            r#"{"name":"@x/lib","exports":{".":{"types":"./src/index.ts","import":"./dist/x.mjs"},"./extra":"./src/extra.ts","./utils/*":"./src/u/*.ts"}}"#,
        ),
        ("packages/lib/src/index.ts", "export const a = 1;\n"),
        ("packages/lib/src/extra.ts", "export const b = 1;\n"),
    ]);
    let api = &w.apis[0];
    assert_eq!(
        api.entrypoints,
        ["packages/lib/src/index.ts", "packages/lib/src/extra.ts"]
    );
    assert!(api.limits[0].contains("./utils/*"));
}

#[test]
fn a_star_re_export_carries_the_names_of_its_target_but_not_default() {
    let w = lib(&[
        (
            "packages/lib/src/index.ts",
            "export * from './all';\nexport const own = 1;\n",
        ),
        (
            "packages/lib/src/all.ts",
            "export const x = 1;\nexport default 2;\nexport * from './deep';\n",
        ),
        ("packages/lib/src/deep.ts", "export const y = 1;\n"),
    ]);
    let mut got = names(&w);
    got.sort_unstable();
    assert_eq!(got, ["own", "x", "y"]);
    // `all.ts` and `deep.ts` are reached through the entrypoint; `all.ts`'s default is the file's
    // own, which no importer names, so it is not listed.
    let a = &w.apis[0];
    assert!(a.unexpectedly_exposed.is_empty());
}

#[test]
fn a_named_re_export_is_followed_to_its_declaration() {
    let w = lib(&[
        (
            "packages/lib/src/index.ts",
            "export { inner as outer } from './inner';\n",
        ),
        ("packages/lib/src/inner.ts", "export const inner = 1;\n"),
    ]);
    let s = &w.apis[0].exported[0];
    assert_eq!(s.name, "outer");
    assert_eq!(
        s.definition,
        Some(("packages/lib/src/inner.ts".to_owned(), "inner".to_owned()))
    );
    assert!(w.apis[0].unexpectedly_exposed.is_empty());
}

#[test]
fn a_cycle_of_re_exports_ends() {
    let w = lib(&[
        ("packages/lib/src/index.ts", "export * from './a';\n"),
        (
            "packages/lib/src/a.ts",
            "export * from './b';\nexport const a = 1;\n",
        ),
        (
            "packages/lib/src/b.ts",
            "export * from './a';\nexport const b = 1;\n",
        ),
    ]);
    let mut got = names(&w);
    got.sort_unstable();
    assert_eq!(got, ["a", "b"]);
}

#[test]
fn an_underscore_name_is_unstable() {
    let w = lib(&[(
        "packages/lib/src/index.ts",
        "export const _x = 1;\nexport const y = 1;\n",
    )]);
    let st: Vec<_> = w.apis[0]
        .exported
        .iter()
        .map(|s| format!("{}:{:?}", s.name, s.stability))
        .collect();
    assert_eq!(st, ["_x:Unstable", "y:Stable"]);
}

#[test]
fn a_module_with_no_file_of_a_language_the_surface_reads_says_so() {
    let w = world(&[
        ("pkg/pubspec.yaml", "name: p\n"),
        ("pkg/lib/a.dart", "class A {}\n"),
        ("packages/x/package.json", r#"{"name":"x"}"#),
        ("packages/x/README.md", "x"),
    ]);
    assert!(w.apis.iter().all(|a| !a.covered));
}

// ─── the public surface of a Rust crate ─────────────────────────────────────────────────────

fn rust(files: &[(&str, &str)]) -> (Vec<String>, Vec<String>) {
    let mut all = vec![
        ("Cargo.toml", "[workspace]\nmembers = [\"crates/a\"]\n"),
        (
            "crates/a/Cargo.toml",
            "[package]\nname = \"a\"\nversion = \"0.1.0\"\n",
        ),
    ];
    all.extend_from_slice(files);
    let w = world(&all);
    let api = w.apis.iter().find(|a| a.module_id == "crates/a").unwrap();
    assert_eq!(api.language, "rust");
    let n = |v: &[debuggatha_engine::analysis::PublicApiSymbol]| {
        let mut x: Vec<String> = v.iter().map(|s| s.name.clone()).collect();
        x.sort_unstable();
        x
    };
    (n(&api.exported), n(&api.unexpectedly_exposed))
}

#[test]
fn only_pub_items_reachable_through_pub_modules_are_the_surface() {
    let (exported, exposed) = rust(&[
        (
            "crates/a/src/lib.rs",
            "pub fn top() {}\nfn private() {}\npub(crate) fn crate_only() {}\npub struct S;\npub enum E {}\npub trait T {}\npub type Alias = u8;\npub const C: u8 = 1;\npub static ST: u8 = 1;\npub mod open;\nmod hidden;\n",
        ),
        (
            "crates/a/src/open.rs",
            "pub fn inside() {}\npub(super) fn nope() {}\n",
        ),
        ("crates/a/src/hidden.rs", "pub fn buried() {}\n"),
    ]);
    assert_eq!(
        exported,
        [
            "Alias",
            "C",
            "E",
            "S",
            "ST",
            "T",
            "open",
            "open::inside",
            "top"
        ]
    );
    assert_eq!(exposed, ["hidden::buried"]);
}

#[test]
fn a_pub_use_makes_a_name_public_from_where_it_is_declared() {
    let (exported, exposed) = rust(&[
        (
            "crates/a/src/lib.rs",
            "mod error;\nmod util;\npub use error::Error;\npub use util::helper as aid;\npub use util::{self as u2};\n",
        ),
        (
            "crates/a/src/error.rs",
            "pub struct Error;\npub fn unused_helper() {}\n",
        ),
        ("crates/a/src/util.rs", "pub fn helper() {}\n"),
    ]);
    assert_eq!(exported, ["Error", "aid", "u2"]);
    assert_eq!(exposed, ["error::unused_helper"]);
}

#[test]
fn a_glob_re_export_and_a_re_export_of_a_re_export_are_followed() {
    let (exported, exposed) = rust(&[
        (
            "crates/a/src/lib.rs",
            "mod inner;\npub use inner::*;\npub mod api { pub use crate::inner::deep::Thing; }\n",
        ),
        ("crates/a/src/inner.rs", "pub fn f() {}\npub mod deep;\n"),
        ("crates/a/src/inner/deep.rs", "pub struct Thing;\n"),
    ]);
    // `pub use inner::*` brings `f` and the `pub mod deep`, and with it `deep::Thing`.
    assert_eq!(exported, ["api", "api::Thing", "deep", "deep::Thing", "f"]);
    assert!(exposed.is_empty());
}

#[test]
fn macro_export_puts_a_macro_at_the_root_and_doc_hidden_is_unstable_and_tests_are_skipped() {
    let w = world(&[
        ("Cargo.toml", "[workspace]\nmembers = [\"crates/a\"]\n"),
        ("crates/a/Cargo.toml", "[package]\nname = \"a\"\n"),
        (
            "crates/a/src/lib.rs",
            "mod m { #[macro_export] macro_rules! mk { () => {} } }\n#[doc(hidden)]\npub fn internal() {}\n#[cfg(test)]\nmod tests { pub fn t() {} }\n",
        ),
    ]);
    let api = w.apis.iter().find(|a| a.module_id == "crates/a").unwrap();
    let got: Vec<_> = api
        .exported
        .iter()
        .map(|s| format!("{}:{:?}", s.name, s.stability))
        .collect();
    assert_eq!(got, ["mk:Stable", "internal:Unstable"]);
}

#[test]
fn inline_modules_and_path_attributes_are_followed() {
    let (exported, _) = rust(&[
        (
            "crates/a/src/lib.rs",
            "pub mod a { pub mod b { pub fn f() {} } }\n#[path = \"weird/place.rs\"]\npub mod p;\n",
        ),
        ("crates/a/src/weird/place.rs", "pub fn g() {}\n"),
    ]);
    assert_eq!(exported, ["a", "a::b", "a::b::f", "p", "p::g"]);
}

#[test]
fn a_binary_crate_has_no_surface() {
    let w = world(&[
        ("Cargo.toml", "[workspace]\nmembers = [\"crates/a\"]\n"),
        ("crates/a/Cargo.toml", "[package]\nname = \"a\"\n"),
        ("crates/a/src/main.rs", "pub fn f() {}\nfn main() {}\n"),
    ]);
    let api = w.apis.iter().find(|a| a.module_id == "crates/a").unwrap();
    assert!(!api.covered);
    assert!(api.exported.is_empty());
}

#[test]
fn a_rust_surface_is_never_called_dead_by_the_javascript_stage() {
    let w = world(&[
        ("Cargo.toml", "[workspace]\nmembers = [\"crates/a\"]\n"),
        ("crates/a/Cargo.toml", "[package]\nname = \"a\"\n"),
        ("crates/a/src/lib.rs", "pub fn f() {}\n"),
    ]);
    assert!(w.dead.is_empty());
}

// ─── dead code ──────────────────────────────────────────────────────────────────────────────

#[test]
fn an_export_nobody_imports_is_unused_and_one_somebody_imports_is_not() {
    let w = world(&[
        ("packages/lib/package.json", PKG),
        (
            "packages/lib/src/index.ts",
            "export const used = 1;\nexport const unused = 1;\n",
        ),
        (
            "packages/app/package.json",
            r#"{"name":"@x/app","private":true}"#,
        ),
        (
            "packages/app/src/main.ts",
            "import { used } from '@x/lib';\nconsole.log(used);\n",
        ),
    ]);
    assert_eq!(dead(&w, DeadCodeKind::ExportedButUnused), ["unused"]);
    let f = &w.dead[0];
    assert_eq!(f.confidence, Confidence::Medium);
    assert_eq!(f.line, 2);
}

#[test]
fn the_same_word_somewhere_else_is_not_a_use() {
    // The TypeScript engine counted any occurrence of the word.
    let w = lib(&[
        ("packages/lib/src/index.ts", "export const config = 1;\n"),
        (
            "packages/lib/src/other.ts",
            "const config = 2;\nconsole.log(config, 'config');\n",
        ),
    ]);
    assert_eq!(dead(&w, DeadCodeKind::ExportedButUnused), ["config"]);
}

#[test]
fn a_use_through_a_barrel_reaches_the_declaration() {
    let w = world(&[
        ("packages/lib/package.json", PKG),
        (
            "packages/lib/src/index.ts",
            "export { a } from './a';\nexport { b } from './b';\n",
        ),
        ("packages/lib/src/a.ts", "export const a = 1;\n"),
        ("packages/lib/src/b.ts", "export const b = 1;\n"),
        (
            "packages/app/package.json",
            r#"{"name":"@x/app","private":true}"#,
        ),
        (
            "packages/app/src/main.ts",
            "import { a } from '@x/lib';\nconsole.log(a);\n",
        ),
    ]);
    assert_eq!(dead(&w, DeadCodeKind::ExportedButUnused), ["b"]);
}

#[test]
fn a_renamed_import_uses_the_original_name() {
    let w = world(&[
        ("packages/lib/package.json", PKG),
        (
            "packages/lib/src/index.ts",
            "export const a = 1;\nexport const z = 1;\n",
        ),
        (
            "packages/app/package.json",
            r#"{"name":"@x/app","private":true}"#,
        ),
        (
            "packages/app/src/main.ts",
            "import { a as z } from '@x/lib';\nconsole.log(z);\n",
        ),
    ]);
    assert_eq!(dead(&w, DeadCodeKind::ExportedButUnused), ["z"]);
}

#[test]
fn a_namespace_import_uses_the_members_it_reads() {
    let w = world(&[
        ("packages/lib/package.json", PKG),
        (
            "packages/lib/src/index.ts",
            "export const a = 1;\nexport const b = 1;\nexport type T = 1;\nexport const c = 1;\n",
        ),
        (
            "packages/app/package.json",
            r#"{"name":"@x/app","private":true}"#,
        ),
        (
            "packages/app/src/main.ts",
            "import * as lib from '@x/lib';\nlet t: lib.T;\nconsole.log(lib.a, lib['b']);\n",
        ),
    ]);
    assert_eq!(dead(&w, DeadCodeKind::ExportedButUnused), ["c"]);
}

#[test]
fn a_namespace_that_is_passed_on_uses_everything() {
    let w = world(&[
        ("packages/lib/package.json", PKG),
        (
            "packages/lib/src/index.ts",
            "export const a = 1;\nexport const b = 1;\n",
        ),
        (
            "packages/app/package.json",
            r#"{"name":"@x/app","private":true}"#,
        ),
        (
            "packages/app/src/main.ts",
            "import * as lib from '@x/lib';\nregister(lib);\n",
        ),
    ]);
    assert!(dead(&w, DeadCodeKind::ExportedButUnused).is_empty());
}

#[test]
fn a_namespace_read_with_a_computed_key_uses_everything() {
    let w = world(&[
        ("packages/lib/package.json", PKG),
        (
            "packages/lib/src/index.ts",
            "export const a = 1;\nexport const b = 1;\n",
        ),
        (
            "packages/app/package.json",
            r#"{"name":"@x/app","private":true}"#,
        ),
        (
            "packages/app/src/main.ts",
            "import * as lib from '@x/lib';\nconsole.log(lib[k]);\n",
        ),
    ]);
    assert!(dead(&w, DeadCodeKind::ExportedButUnused).is_empty());
}

#[test]
fn a_local_of_the_same_name_does_not_hide_the_namespace_member() {
    let w = world(&[
        ("packages/lib/package.json", PKG),
        ("packages/lib/src/index.ts", "export const a = 1;\n"),
        (
            "packages/app/package.json",
            r#"{"name":"@x/app","private":true}"#,
        ),
        (
            "packages/app/src/main.ts",
            "import * as lib from '@x/lib';\nfunction f(lib) { return lib.a; }\nf({});\n",
        ),
    ]);
    // `lib.a` in `f` reads the parameter, not the import: `a` is not used.
    assert_eq!(dead(&w, DeadCodeKind::ExportedButUnused), ["a"]);
}

#[test]
fn a_deep_import_or_a_dynamic_one_uses_what_it_could_reach() {
    let w = world(&[
        ("packages/lib/package.json", PKG),
        ("packages/lib/src/index.ts", "export const a = 1;\n"),
        ("packages/lib/src/extra.ts", "export const e = 1;\n"),
        (
            "packages/app/package.json",
            r#"{"name":"@x/app","private":true}"#,
        ),
        (
            "packages/app/src/main.ts",
            "import { e } from '@x/lib/src/extra';\nconsole.log(e);\n",
        ),
    ]);
    // The path names a file: only that file's `e` is used.
    assert_eq!(dead(&w, DeadCodeKind::ExportedButUnused), ["a"]);
    assert!(dead(&w, DeadCodeKind::ProbablyDead).is_empty());

    // A path no file answers to could reach any of them.
    let w = world(&[
        ("packages/lib/package.json", PKG),
        ("packages/lib/src/index.ts", "export const a = 1;\n"),
        (
            "packages/app/package.json",
            r#"{"name":"@x/app","private":true}"#,
        ),
        (
            "packages/app/src/main.ts",
            "import { z } from '@x/lib/generated/z';\nconsole.log(z);\n",
        ),
    ]);
    assert!(dead(&w, DeadCodeKind::ExportedButUnused).is_empty());

    let w = lib(&[
        ("packages/lib/src/index.ts", "export const a = 1;\n"),
        (
            "packages/lib/src/load.ts",
            "export const load = () => import('./index');\n",
        ),
    ]);
    assert!(!dead(&w, DeadCodeKind::ExportedButUnused).contains(&"a".to_owned()));
}

#[test]
fn a_computed_import_in_the_module_lowers_confidence() {
    let w = lib(&[
        ("packages/lib/src/index.ts", "export const a = 1;\n"),
        (
            "packages/lib/src/plug.ts",
            "export const load = (n) => import(n);\n",
        ),
    ]);
    let f = w.dead.iter().find(|d| d.symbol == "a").unwrap();
    assert_eq!(f.confidence, Confidence::Low);
    assert!(f.rationale.contains("computed"));
}

#[test]
fn a_publishable_package_is_low_confidence() {
    let w = world(&[
        (
            "packages/lib/package.json",
            r#"{"name":"@x/lib","main":"src/index.ts"}"#,
        ),
        ("packages/lib/src/index.ts", "export const a = 1;\n"),
    ]);
    let f = &w.dead[0];
    assert_eq!(f.confidence, Confidence::Low);
    assert!(f.rationale.contains("publishable"));
}

#[test]
fn an_internal_export_nobody_reads_is_probably_dead_and_one_read_in_its_file_is_not() {
    let w = lib(&[
        ("packages/lib/src/index.ts", "export const api = 1;\n"),
        (
            "packages/lib/src/inner.ts",
            "export const dead1 = 1;\nexport const local = 1;\nconsole.log(local);\nexport const shared = 1;\n",
        ),
        (
            "packages/lib/src/user.ts",
            "import { shared } from './inner';\nconsole.log(shared);\n",
        ),
    ]);
    assert_eq!(dead(&w, DeadCodeKind::ProbablyDead), ["dead1"]);
    assert!(
        w.dead
            .iter()
            .all(|d| d.symbol != "local" && d.symbol != "shared")
    );
    // It is still an exposure the surface reports.
    assert!(
        w.apis[0]
            .unexpectedly_exposed
            .iter()
            .any(|s| s.name == "local")
    );
}

#[test]
fn a_declaration_nothing_reads_is_unreferenced_and_the_scopes_decide() {
    let src = "\
function unusedFn() {}
class UnusedClass {}
interface UnusedI {}
type UnusedT = 1;
const unusedVar = compute();
function used() {}
const shadow = 1;
function g(shadow) { return shadow; }
used();
g(1);
export const keep = 1;
";
    let w = lib(&[("packages/lib/src/a.ts", src)]);
    let mut got: Vec<_> = w
        .dead
        .iter()
        .filter(|d| d.kind == DeadCodeKind::Unreferenced)
        .map(|d| (d.symbol.as_str(), d.confidence))
        .collect();
    got.sort_unstable_by_key(|(n, _)| *n);
    assert_eq!(
        got,
        [
            ("UnusedClass", Confidence::High),
            ("UnusedI", Confidence::High),
            ("UnusedT", Confidence::High),
            // `shadow` at the top is never read: the parameter of `g` is another name.
            ("shadow", Confidence::Medium),
            ("unusedFn", Confidence::High),
            ("unusedVar", Confidence::Medium),
        ]
    );
}

#[test]
fn what_is_exported_imported_decorated_or_underscored_is_never_unreferenced() {
    let src = "\
import { thing } from './t';
@Injectable()
class Service {}
const _scratch = 1;
declare const ambient: number;
function exportedLater() {}
export { exportedLater };
export default class Main {}
";
    let w = lib(&[("packages/lib/src/a.ts", src)]);
    assert!(dead(&w, DeadCodeKind::Unreferenced).is_empty());
}

#[test]
fn a_script_file_has_global_names_other_files_may_read() {
    let w = lib(&[(
        "packages/lib/src/globals.js",
        "function helper() {}\nvar shared = 1;\n",
    )]);
    assert!(dead(&w, DeadCodeKind::Unreferenced).is_empty());
}

#[test]
fn a_component_used_in_jsx_is_a_use() {
    let src = "function Used() { return null; }\nfunction Unused() { return null; }\nexport const App = () => <Used />;\n";
    let w = lib(&[("packages/lib/src/a.tsx", src)]);
    assert_eq!(dead(&w, DeadCodeKind::Unreferenced), ["Unused"]);
}

#[test]
fn a_single_file_component_declares_nothing_a_report_can_name() {
    let src = "<script setup>\nimport * as ns from './m';\nfunction helper() {}\n</script>\n<template>{{ ns.a }}</template>\n";
    let w = lib(&[
        ("packages/lib/src/index.ts", "export const x = 1;\n"),
        ("packages/lib/src/c.vue", src),
        (
            "packages/lib/src/m.ts",
            "export const a = 1;\nexport const b = 1;\n",
        ),
    ]);
    assert!(w.dead.iter().all(|d| d.file != "packages/lib/src/c.vue"));
    // The template reads `ns`: nothing of `m` is reported.
    assert!(w.dead.iter().all(|d| d.file != "packages/lib/src/m.ts"));
}

// ─── ownership ──────────────────────────────────────────────────────────────────────────────

/// The owners of each file, one boundary per file so a module does not blur the answer.
fn owners_of(codeowners: &str, files: &[&str]) -> Vec<Vec<String>> {
    let mut all: Vec<(String, String)> = vec![(".github/CODEOWNERS".into(), codeowners.into())];
    all.extend(files.iter().map(|f| ((*f).to_owned(), String::new())));
    let ws = Workspace::from_files(all);
    let boundaries: Vec<ModuleBoundary> = files
        .iter()
        .map(|f| ModuleBoundary {
            id: (*f).to_owned(),
            kind: ModuleKind::Domain,
            name: None,
            manifest: Manifest::None,
            files: vec![(*f).to_owned()],
        })
        .collect();
    detect_ownership(&ws, &boundaries)
        .into_iter()
        .map(|o| o.owners)
        .collect()
}

fn one(owner: &str) -> Vec<String> {
    vec![owner.to_owned()]
}

#[test]
fn the_last_matching_rule_wins_and_an_empty_rule_takes_ownership_away() {
    let owners = owners_of(
        "* @all\n/packages/a/ @alpha\n/packages/b/ @beta\n/packages/b/\n",
        &["packages/a/x.ts", "packages/b/y.ts", "packages/c/z.ts"],
    );
    assert_eq!(owners, [one("@alpha"), vec![], one("@all")]);
}

#[test]
fn a_pattern_with_no_slash_matches_at_any_depth_and_one_with_a_slash_does_not() {
    let owners = owners_of(
        "*.ts @ts\n/docs/* @docs\nsrc/ @src\nREADME.md @readme\n",
        &[
            "packages/a/deep/x.ts",
            "docs/a.md",
            "docs/sub/b.md",
            "other/src/c.js",
            "src/d.js",
            "a/b/README.md",
            "packages/a/deep/x.md",
        ],
    );
    assert_eq!(
        owners,
        [
            one("@ts"),
            one("@docs"),
            // `/docs/*` is the files directly in `docs`.
            vec![],
            // `src/` is a directory named src at any depth.
            one("@src"),
            one("@src"),
            one("@readme"),
            vec![],
        ]
    );
}

#[test]
fn double_star_crosses_directories_and_star_does_not() {
    let owners = owners_of(
        "/a/**/gen.ts @gen\n/a/*.ts @flat\n/b/** @all\n",
        &[
            "a/gen.ts",
            "a/b/c/gen.ts",
            "a/b/x.ts",
            "a/y.ts",
            "b",
            "b/z.ts",
        ],
    );
    assert_eq!(
        owners,
        [
            // Both match; the later rule wins.
            one("@flat"),
            one("@gen"),
            vec![],
            one("@flat"),
            // `/b/**` is what is inside `b`, not `b` itself.
            vec![],
            one("@all"),
        ]
    );
}

#[test]
fn a_module_reports_the_owners_of_its_files_and_when_they_differ() {
    let ws = Workspace::from_files([
        (".github/CODEOWNERS", "/a/x.ts @one\n/a/y.ts @two\n"),
        ("a/x.ts", ""),
        ("a/y.ts", ""),
        ("a/z.ts", ""),
    ]);
    let b = ModuleBoundary {
        id: "a".into(),
        kind: ModuleKind::Domain,
        name: None,
        manifest: Manifest::None,
        files: vec!["a/x.ts".into(), "a/y.ts".into(), "a/z.ts".into()],
    };
    let o = &detect_ownership(&ws, &[b])[0];
    assert_eq!(o.owners, ["@one", "@two"]);
    assert_eq!((o.owned_files, o.total_files, o.uniform), (2, 3, false));
}

#[test]
fn the_github_folder_wins_over_the_root() {
    let ws = Workspace::from_files([
        ("CODEOWNERS", "* @root\n"),
        (".github/CODEOWNERS", "* @github\n"),
        ("a.ts", ""),
    ]);
    let b = ModuleBoundary {
        id: "a.ts".into(),
        kind: ModuleKind::Domain,
        name: None,
        manifest: Manifest::None,
        files: vec!["a.ts".into()],
    };
    assert_eq!(detect_ownership(&ws, &[b])[0].owners, ["@github"]);
}

#[test]
fn comments_sections_and_owner_comments_are_not_owners() {
    let owners = owners_of(
        "# comment\n[Docs]\n/packages/a/ @alpha # the a team\n",
        &["packages/a/x.ts"],
    );
    assert_eq!(owners, [one("@alpha")]);
}

#[test]
fn no_codeowners_is_unknown() {
    let ws = Workspace::from_files([("packages/a/src/x.ts", "")]);
    let found = infer_boundaries(&ws);
    let o = detect_ownership(&ws, &found.list);
    assert!(o.iter().all(|s| s.owners.is_empty()));
}

// ─── change impact ──────────────────────────────────────────────────────────────────────────

fn impact(changed: &[&str]) -> debuggatha_engine::analysis::ChangeImpact {
    let files = [
        (
            "packages/lib/package.json",
            r#"{"name":"@x/lib","private":true,"main":"src/index.ts"}"#,
        ),
        ("packages/lib/src/index.ts", "export const api = 1;\n"),
        ("packages/lib/src/inner.ts", "export const i = 1;\n"),
        (
            "packages/mid/package.json",
            r#"{"name":"@x/mid","private":true}"#,
        ),
        (
            "packages/mid/src/m.ts",
            "import { api } from '@x/lib';\nexport const m = api;\n",
        ),
        (
            "packages/top/package.json",
            r#"{"name":"@x/top","private":true}"#,
        ),
        (
            "packages/top/src/t.ts",
            "import { m } from '@x/mid';\nconsole.log(m);\n",
        ),
        (
            "packages/lone/package.json",
            r#"{"name":"@x/lone","private":true}"#,
        ),
        ("packages/lone/src/l.ts", "export const l = 1;\n"),
    ];
    let w = world(&files);
    let found = infer_boundaries(&w.ws);
    let graph = build_graph(&w.ws, &found);
    let changed: Vec<String> = changed.iter().map(|s| (*s).to_owned()).collect();
    estimate_change_impact(&changed, &found.list, &graph, &w.apis)
}

#[test]
fn dependents_are_found_transitively() {
    let i = impact(&["packages/lib/src/inner.ts"]);
    assert_eq!(i.affected_modules, ["packages/lib"]);
    assert_eq!(i.downstream_modules, ["packages/mid", "packages/top"]);
    assert_eq!(i.confidence, ImpactConfidence::High);
}

#[test]
fn a_change_to_a_public_name_is_medium_and_names_it() {
    let i = impact(&["packages/lib/src/index.ts"]);
    assert_eq!(i.affected_public_api.len(), 1);
    assert_eq!(i.affected_public_api[0].name, "api");
    assert_eq!(i.confidence, ImpactConfidence::Medium);
}

#[test]
fn a_leaf_has_no_dependents() {
    let i = impact(&["packages/lone/src/l.ts"]);
    assert!(i.downstream_modules.is_empty());
}

#[test]
fn a_file_no_module_owns_is_listed_and_lowers_the_estimate() {
    let i = impact(&["packages/lone/src/l.ts", "scripts/deploy.py"]);
    assert_eq!(i.unmapped_files, ["scripts/deploy.py"]);
    assert_eq!(i.confidence, ImpactConfidence::Medium);
    let only = impact(&["scripts/deploy.py"]);
    assert_eq!(only.confidence, ImpactConfidence::Low);
    assert!(only.affected_modules.is_empty());
}

// ─── names ────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────

#[test]
fn a_question_mark_is_one_character_of_a_name() {
    let owners = owners_of("/a/?.ts @one\n", &["a/x.ts", "a/xy.ts", "a/.ts"]);
    assert_eq!(owners, [one("@one"), vec![], vec![]]);
}

#[test]
fn a_name_exported_with_braces_and_read_nowhere_is_dead_and_one_read_in_its_file_is_not() {
    let w = lib(&[
        ("packages/lib/src/index.ts", "export const api = 1;\n"),
        (
            "packages/lib/src/inner.ts",
            "function gone() {}\nfunction kept() {}\nkept();\nexport { gone, kept };\n",
        ),
    ]);
    assert_eq!(dead(&w, DeadCodeKind::ProbablyDead), ["gone"]);
    // `kept` is read in its own file: not dead, and not this report's business.
    assert_eq!(dead(&w, DeadCodeKind::ExportedButUnused), ["api"]);
}

#[test]
fn an_enum_nothing_reads_is_medium_because_it_has_a_runtime_object() {
    let w = lib(&[(
        "packages/lib/src/a.ts",
        "enum Color { Red }\nexport const x = 1;\n",
    )]);
    let f = w.dead.iter().find(|d| d.symbol == "Color").unwrap();
    assert_eq!(
        (f.kind, f.confidence),
        (DeadCodeKind::Unreferenced, Confidence::Medium)
    );
}

#[test]
fn conditions_are_tried_in_the_order_types_import_default() {
    let w = world(&[
        (
            "packages/lib/package.json",
            r#"{"name":"@x/lib","exports":{"import":"./src/esm.ts","types":"./src/types.ts","require":"./src/cjs.ts"}}"#,
        ),
        ("packages/lib/src/esm.ts", "export const e = 1;\n"),
        ("packages/lib/src/types.ts", "export const t = 1;\n"),
        ("packages/lib/src/cjs.ts", "export const c = 1;\n"),
    ]);
    assert_eq!(w.apis[0].entrypoints, ["packages/lib/src/types.ts"]);
}

#[test]
fn a_build_folder_other_than_dist_is_read_as_the_source_too() {
    for out in ["build", "lib", "out"] {
        let manifest = format!(r#"{{"name":"@x/lib","main":"{out}/index.js"}}"#);
        let w = world(&[
            ("packages/lib/package.json", &manifest),
            ("packages/lib/src/index.ts", "export const a = 1;\n"),
        ]);
        assert_eq!(
            w.apis[0].entrypoints,
            ["packages/lib/src/index.ts"],
            "{out}"
        );
    }
}

#[test]
fn a_change_to_the_file_a_name_is_declared_in_touches_the_public_api() {
    let files = [
        (
            "packages/lib/package.json",
            r#"{"name":"@x/lib","private":true,"main":"src/index.ts"}"#,
        ),
        (
            "packages/lib/src/index.ts",
            "export { i } from './inner';\n",
        ),
        ("packages/lib/src/inner.ts", "export const i = 1;\n"),
    ];
    let w = world(&files);
    let found = infer_boundaries(&w.ws);
    let graph = build_graph(&w.ws, &found);
    let r = estimate_change_impact(
        &["packages/lib/src/inner.ts".to_owned()],
        &found.list,
        &graph,
        &w.apis,
    );
    assert_eq!(r.affected_public_api.len(), 1);
    assert_eq!(r.affected_public_api[0].name, "i");
}

#[test]
fn a_changed_file_the_graph_could_not_read_is_said() {
    let files = [
        (
            "packages/lib/package.json",
            r#"{"name":"@x/lib","private":true}"#,
        ),
        ("packages/lib/src/broken.ts", "export const = ;\n"),
    ];
    let w = world(&files);
    let found = infer_boundaries(&w.ws);
    let graph = build_graph(&w.ws, &found);
    let r = estimate_change_impact(
        &["packages/lib/src/broken.ts".to_owned()],
        &found.list,
        &graph,
        &w.apis,
    );
    assert_eq!(r.confidence, ImpactConfidence::Medium);
    assert!(r.rationale.contains("could not be read"));
}

#[test]
fn a_default_export_of_a_file_that_is_not_an_entrypoint_is_not_an_exposure() {
    let w = lib(&[
        ("packages/lib/src/index.ts", "export const api = 1;\n"),
        (
            "packages/lib/src/settings.ts",
            "export default { a: 1 };\nexport const named = 1;\n",
        ),
    ]);
    let names: Vec<_> = w.apis[0]
        .unexpectedly_exposed
        .iter()
        .map(|s| s.name.as_str())
        .collect();
    assert_eq!(names, ["named"]);
}

#[test]
fn what_a_tool_loads_by_name_exposes_nothing() {
    let w = lib(&[
        ("packages/lib/src/index.ts", "export const api = 1;\n"),
        ("packages/lib/vite.config.ts", "export const helper = 1;\n"),
        (
            "packages/lib/src/routes/+page.ts",
            "export const load = 1;\n",
        ),
        ("packages/lib/src/a.test.ts", "export const fixture = 1;\n"),
        ("packages/lib/tests/util.ts", "export const util = 1;\n"),
        (
            "packages/lib/src/hooks.server.ts",
            "export const handle = 1;\n",
        ),
    ]);
    assert!(w.apis[0].unexpectedly_exposed.is_empty());
    assert_eq!(dead(&w, DeadCodeKind::ProbablyDead), Vec::<String>::new());
}

#[test]
fn an_imported_name_exported_again_is_a_re_export_not_a_declaration() {
    let w = world(&[
        ("packages/lib/package.json", PKG),
        (
            "packages/lib/src/index.ts",
            "import { a } from './a';\nimport * as ns from './b';\nimport d from './d';\nexport { a, ns, d as dflt };\n",
        ),
        ("packages/lib/src/a.ts", "export const a = 1;\n"),
        ("packages/lib/src/b.ts", "export const b = 1;\n"),
        ("packages/lib/src/d.ts", "export default 1;\n"),
    ]);
    let api = &w.apis[0];
    assert!(api.unexpectedly_exposed.is_empty());
    let a = api.exported.iter().find(|s| s.name == "a").unwrap();
    assert_eq!(
        a.definition,
        Some(("packages/lib/src/a.ts".to_owned(), "a".to_owned()))
    );
    let d = api.exported.iter().find(|s| s.name == "dflt").unwrap();
    assert_eq!(
        d.definition,
        Some(("packages/lib/src/d.ts".to_owned(), "default".to_owned()))
    );
    // Nothing imports from the barrel: passing a name on is not a use.
    let mut dead_names: Vec<_> = dead(&w, DeadCodeKind::ExportedButUnused);
    dead_names.sort_unstable();
    assert_eq!(dead_names, ["a", "dflt", "ns"]);
}

#[test]
fn a_custom_condition_names_the_source_when_the_standard_ones_name_a_build() {
    let w = world(&[
        (
            "packages/lib/package.json",
            r#"{"name":"@x/lib","exports":{".":{"@x/source":"./src/index.ts","types":"./index.d.cts","import":"./index.js"},"./v3":{"@x/source":"./src/v3/index.ts","types":"./v3/index.d.cts"}}}"#,
        ),
        ("packages/lib/src/index.ts", "export const a = 1;\n"),
        ("packages/lib/src/v3/index.ts", "export const v = 1;\n"),
    ]);
    let api = &w.apis[0];
    assert_eq!(
        api.subpaths,
        [
            (".".to_owned(), "packages/lib/src/index.ts".to_owned()),
            ("./v3".to_owned(), "packages/lib/src/v3/index.ts".to_owned()),
        ]
    );
}

#[test]
fn an_import_of_a_subpath_uses_what_the_manifest_gives_that_subpath() {
    let w = world(&[
        (
            "packages/lib/package.json",
            r#"{"name":"@x/lib","private":true,"exports":{".":"./src/index.ts","./v3":"./src/v3.ts"}}"#,
        ),
        ("packages/lib/src/index.ts", "export const a = 1;\n"),
        (
            "packages/lib/src/v3.ts",
            "export const v = 1;\nexport const w = 1;\n",
        ),
        (
            "packages/app/package.json",
            r#"{"name":"@x/app","private":true}"#,
        ),
        (
            "packages/app/src/main.ts",
            "import { v } from '@x/lib/v3';\nconsole.log(v);\n",
        ),
    ]);
    assert_eq!(dead(&w, DeadCodeKind::ExportedButUnused), ["a", "w"]);
}

#[test]
fn importing_a_stylesheet_of_a_package_uses_no_name() {
    let w = world(&[
        ("packages/lib/package.json", PKG),
        ("packages/lib/src/index.ts", "export const a = 1;\n"),
        (
            "packages/app/package.json",
            r#"{"name":"@x/app","private":true}"#,
        ),
        ("packages/app/src/main.ts", "import '@x/lib/tokens.css';\n"),
    ]);
    assert_eq!(dead(&w, DeadCodeKind::ExportedButUnused), ["a"]);
}

#[test]
fn a_name_imported_through_an_alias_nobody_resolves_is_not_called_dead() {
    let w = world(&[
        ("packages/lib/package.json", PKG),
        ("packages/lib/src/index.ts", "export const api = 1;\n"),
        (
            "packages/lib/src/inner.ts",
            "export const viaAlias = 1;\nexport const other = 1;\n",
        ),
        (
            "packages/lib/src/user.ts",
            "import { viaAlias } from '#app/inner';\nconsole.log(viaAlias);\n",
        ),
    ]);
    assert_eq!(dead(&w, DeadCodeKind::ProbablyDead), ["other"]);
}

#[test]
fn files_under_pages_routes_and_a_framework_app_folder_are_loaded_by_name() {
    for path in [
        "packages/lib/src/pages/home.ts",
        "packages/lib/src/routes/x.ts",
        "packages/lib/app/blog/page.tsx",
        "packages/lib/app/route.ts",
    ] {
        let w = lib(&[
            ("packages/lib/src/index.ts", "export const api = 1;\n"),
            (path, "export const revalidate = false;\n"),
        ]);
        assert!(w.apis[0].unexpectedly_exposed.is_empty(), "{path}");
    }
    let w = lib(&[
        ("packages/lib/src/index.ts", "export const api = 1;\n"),
        ("packages/lib/src/page.ts", "export const x = 1;\n"),
    ]);
    assert_eq!(w.apis[0].unexpectedly_exposed.len(), 1);
}

#[test]
fn super_and_self_paths_in_a_pub_use_are_followed() {
    let (exported, exposed) = rust(&[
        ("crates/a/src/lib.rs", "mod inner;\npub mod api;\n"),
        (
            "crates/a/src/inner.rs",
            "pub struct Hidden;\npub struct Other;\n",
        ),
        (
            "crates/a/src/api.rs",
            "pub use super::inner::Hidden;\npub use self::sub::Leaf;\npub mod sub { pub struct Leaf; }\n",
        ),
    ]);
    assert_eq!(
        exported,
        [
            "api",
            "api::Hidden",
            "api::Leaf",
            "api::sub",
            "api::sub::Leaf"
        ]
    );
    assert_eq!(exposed, ["inner::Other"]);
}

#[test]
fn a_pub_item_inside_a_pub_mod_of_a_private_mod_is_still_unreachable() {
    let (exported, exposed) = rust(&[
        ("crates/a/src/lib.rs", "mod private;\npub fn top() {}\n"),
        (
            "crates/a/src/private.rs",
            "pub mod open { pub fn deep() {} }\n",
        ),
    ]);
    assert_eq!(exported, ["top"]);
    assert_eq!(exposed, ["private::open", "private::open::deep"]);
}
