//! The paths a file uses and the module tree of a crate. The cases are the
//! specification; the declared edges were also compared with `cargo metadata` on the corpus.
#![allow(clippy::expect_used, clippy::unwrap_used)]
// a failed expect is a failed test
#![allow(clippy::too_many_lines)] // tables of cases

use debuggatha_engine::analysis::rust::read_rust;
use debuggatha_engine::analysis::{Workspace, build_graph, infer_boundaries};

fn paths(src: &str) -> Vec<String> {
    read_rust(src)
        .expect("parses")
        .refs
        .into_iter()
        .filter(|r| !r.decl)
        .map(|r| r.segments.join("::"))
        .collect()
}

#[test]
fn every_way_to_name_a_path_is_found() {
    let src = "\
use crate::a::b;
use self::x;
use super::y;
use super::super::z;
use crate::{c, d::{e, f}, g::h as i};
use j::{self, k};
use l::m::*;
use n::o as p;
extern crate q as r;
fn f() {
    let v = crate::s::t();
    let w: crate::u::Ty = std::fs::read();
    let n = Vec::new();
}
";
    assert_eq!(
        paths(src),
        [
            "crate::a::b",
            "self::x",
            "super::y",
            "super::super::z",
            "crate::c",
            "crate::d::e",
            "crate::d::f",
            "crate::g::h",
            "j",
            "j::k",
            "l::m",
            "n::o",
            "q",
            "crate::s::t",
            "crate::u::Ty",
            "std::fs::read",
            "Vec::new",
        ]
    );
}

#[test]
fn text_and_macro_bodies_are_not_paths() {
    let src = "\
// use crate::in_comment;
/* use crate::in_block; */
const S: &str = \"crate::in_string\";
fn f() {
    println!(\"{}\", crate::in_macro::f());
    let _ = <T as Trait>::g();
}
";
    // Macro arguments are tokens to the parser: a path written there is not seen (stated limit).
    assert_eq!(paths(src), Vec::<String>::new());
}

#[test]
fn declarations_and_the_inline_modules_a_path_sits_in() {
    let parsed = read_rust(
        "mod a;\n#[path = \"x/y.rs\"]\nmod b;\nmod c {\n    mod d;\n    use super::e;\n}\n",
    )
    .unwrap();
    let decls: Vec<_> = parsed
        .mods
        .iter()
        .map(|m| {
            (
                m.name.as_str(),
                m.path_attr.as_deref(),
                m.body.as_ref().map(Vec::len),
            )
        })
        .collect();
    assert_eq!(
        decls,
        [
            ("a", None, None),
            ("b", Some("x/y.rs"), None),
            ("c", None, Some(1))
        ]
    );
    let refs: Vec<_> = parsed
        .refs
        .iter()
        .map(|r| (r.segments.join("::"), r.decl, r.inline.clone(), r.line))
        .collect();
    assert_eq!(
        refs,
        [
            ("a".to_owned(), true, vec![], 1),
            ("b".to_owned(), true, vec![], 3),
            ("d".to_owned(), true, vec!["c".to_owned()], 5),
            ("super::e".to_owned(), false, vec!["c".to_owned()], 6),
        ]
    );
}

fn ws(files: &[(&str, &str)]) -> Workspace {
    Workspace::from_files(files.iter().map(|(p, c)| (*p, *c)))
}

fn shape(w: &Workspace) -> Vec<String> {
    let g = build_graph(w, &infer_boundaries(w));
    g.edges
        .iter()
        .map(|e| {
            let ev: Vec<String> = e
                .evidence
                .iter()
                .map(|v| format!("{}:{}", v.file, v.detail))
                .collect();
            format!("{} > {} {:?} [{}]", e.from, e.to, e.kind, ev.join(" | "))
        })
        .collect()
}

#[test]
fn modules_resolve_through_mod_files_paths_and_inline_modules() {
    let w = ws(&[
        ("Cargo.toml", "[package]\nname = \"one\"\n"),
        (
            "src/lib.rs",
            "mod a;\nmod b;\n#[path = \"extra/p.rs\"]\nmod p;\nmod outer {\n    mod inner;\n}\n",
        ),
        (
            "src/a/mod.rs",
            "use crate::b::helper;\nuse super::b::other;\nuse self::deep::Thing;\nmod deep;\n",
        ),
        ("src/a/deep.rs", "use super::super::b::x::Y;\n"),
        ("src/b.rs", "mod x;\npub fn helper() {}\n"),
        ("src/b/x.rs", "pub struct Y;\n"),
        ("src/extra/p.rs", "use crate::a::deep::Thing;\n"),
        ("src/outer/inner.rs", "use crate::b::helper;\n"),
    ]);
    assert_eq!(
        shape(&w),
        [
            "src/a > src/b Import [src/a/deep.rs:imports \"super::super::b::x::Y\" | src/a/mod.rs:imports \"crate::b::helper\" | src/a/mod.rs:imports \"super::b::other\"]",
            "src/extra > src/a Import [src/extra/p.rs:imports \"crate::a::deep::Thing\"]",
            "src/outer > src/b Import [src/outer/inner.rs:imports \"crate::b::helper\"]",
        ]
    );
}

#[test]
fn a_file_module_with_children_looks_for_them_in_its_own_directory() {
    let w = ws(&[
        ("Cargo.toml", "[package]\nname = \"one\"\n"),
        ("src/main.rs", "mod a;\nmod z;\n"),
        ("src/a.rs", "mod sub;\n"),
        ("src/a/sub.rs", "use crate::z::W;\n"),
        ("src/z/w.rs", ""),
        ("src/z.rs", "mod w;\npub use w::W;\n"),
    ]);
    // `sub` is `src/a/sub.rs`, so `crate::z::W` leaves the boundary `src/a` for `src/z`.
    assert_eq!(
        shape(&w),
        ["src/a > src/z Import [src/a/sub.rs:imports \"crate::z::W\"]"]
    );
}

#[test]
fn a_declaration_that_leaves_the_boundary_is_an_edge() {
    let w = ws(&[
        ("Cargo.toml", "[package]\nname = \"one\"\n"),
        ("src/lib.rs", "mod b;\nmod a;\n"),
        ("src/a/mod.rs", "#[path = \"../b/x.rs\"]\nmod xx;\n"),
        ("src/b/mod.rs", "mod x;\n"),
        ("src/b/x.rs", ""),
    ]);
    assert_eq!(
        shape(&w),
        ["src/a > src/b Import [src/a/mod.rs:declares module \"xx\"]"]
    );
}

#[test]
fn a_crate_of_the_workspace_is_named_by_its_name_with_underscores() {
    let w = ws(&[
        ("Cargo.toml", "[workspace]\nmembers = [\"crates/*\"]\n"),
        (
            "crates/app/Cargo.toml",
            "[package]\nname = \"app\"\n[dependencies]\ncore-lib = { path = \"../core\" }\n",
        ),
        (
            "crates/app/src/main.rs",
            "use core_lib::Thing;\nfn f() { let _ = core_lib::go(); let _ = other::x(); }\n",
        ),
        ("crates/core/Cargo.toml", "[package]\nname = \"core-lib\"\n"),
        ("crates/core/src/lib.rs", "pub struct Thing;\n"),
    ]);
    assert_eq!(
        shape(&w),
        [
            "crates/app > crates/core Declared [Cargo.toml:declares dependency on \"core-lib\"]",
            "crates/app > crates/core Import [crates/app/src/main.rs:imports \"core_lib::Thing\" | crates/app/src/main.rs:imports \"core_lib::go\"]",
        ]
    );
}

#[test]
fn a_path_attribute_is_relative_to_the_directory_of_the_file() {
    // In `src/a.rs` (a file module, not `mod.rs`) the attribute still starts at `src/`.
    let w = ws(&[
        ("Cargo.toml", "[package]\nname = \"one\"\n"),
        ("src/main.rs", "mod a;\nmod z;\n"),
        ("src/a.rs", "#[path = \"z/p.rs\"]\nmod p;\n"),
        ("src/a/other.rs", ""),
        ("src/z/p.rs", ""),
        ("src/z.rs", ""),
    ]);
    assert_eq!(
        shape(&w),
        ["src/a > src/z Import [src/a.rs:declares module \"p\"]"]
    );
}

#[test]
fn a_build_script_is_a_root_of_its_own() {
    let w = ws(&[
        ("Cargo.toml", "[package]\nname = \"one\"\n"),
        (
            "build.rs",
            "#[path = \"src/tools/helper.rs\"]\nmod helper;\n#[path = \"src/x/x.rs\"]\nmod x;\n",
        ),
        ("src/tools/helper.rs", "use crate::x::Y;\n"),
        ("src/x/x.rs", ""),
    ]);
    assert_eq!(
        shape(&w),
        ["src/tools > src/x Import [src/tools/helper.rs:imports \"crate::x::Y\"]"]
    );
}

#[test]
fn a_rust_file_no_module_reaches_is_listed_not_guessed() {
    let w = ws(&[
        ("Cargo.toml", "[package]\nname = \"one\"\n"),
        ("src/main.rs", "automod::dir!(\"src/gen\");\nmod a;\n"),
        ("src/a/mod.rs", ""),
        ("src/gen/g.rs", "use crate::a::X;\n"),
    ]);
    let g = build_graph(&w, &infer_boundaries(&w));
    assert_eq!(g.unrooted_files, ["src/gen/g.rs"]);
    assert_eq!(g.edges.len(), 0);
}
