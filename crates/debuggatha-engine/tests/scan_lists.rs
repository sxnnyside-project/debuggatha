//! The two lists of extensions a scan can use: the parity list the gates build their units with (what
//! the TypeScript engine reads) and the list a review of a workspace uses.
#![allow(clippy::expect_used, clippy::unwrap_used)] // a failed expect is a failed test

use std::path::Path;

use debuggatha_engine::scan_scope::{PARITY_EXTENSIONS, SCAN_EXTENSIONS, list_scan_files};
use debuggatha_engine::syntax::Language;

#[test]
fn the_parity_list_is_what_the_typescript_engine_reads() {
    // `SOURCE_EXTENSIONS` in `packages/engine/src/skills/engine/read-files.ts` at tag `ts-engine-final`.
    assert_eq!(
        PARITY_EXTENSIONS,
        [
            "ts", "tsx", "js", "jsx", "mjs", "cjs", "dart", "rs", "go", "py", "kt", "kts", "php",
            "cs", "java", "vue", "svelte"
        ]
    );
}

#[test]
fn the_scan_list_adds_languages_the_engine_parses_and_nothing_else() {
    let added: Vec<_> = SCAN_EXTENSIONS
        .iter()
        .filter(|e| !PARITY_EXTENSIONS.contains(e))
        .collect();
    assert_eq!(
        added,
        [
            &"rb", &"ex", &"exs", &"swift", &"cpp", &"cc", &"cxx", &"hpp", &"hh", &"hxx", &"gd",
            &"astro"
        ]
    );
    for e in PARITY_EXTENSIONS {
        assert!(
            SCAN_EXTENSIONS.contains(e),
            "{e} is in the parity list only"
        );
    }
    // Every extension listed has a parser: a scan never hands the engine a file it cannot read.
    for e in SCAN_EXTENSIONS {
        let path = format!("a.{e}");
        assert!(
            Language::from_path(Path::new(&path)).is_some(),
            "{e} has no parser"
        );
    }
    // The ones left out: nothing reads HTML, and a header does not say which language it is.
    for e in ["html", "h"] {
        assert!(!SCAN_EXTENSIONS.contains(&e));
    }
}

#[test]
fn a_workspace_scan_lists_the_added_languages_and_the_parity_scan_does_not() {
    let dir = std::env::temp_dir().join(format!("debuggatha-scan-{}", std::process::id()));
    drop(std::fs::remove_dir_all(&dir));
    std::fs::create_dir_all(&dir).unwrap();
    for f in [
        "a.rs", "b.rb", "c.ex", "d.exs", "e.swift", "f.cpp", "g.hpp", "h.gd", "i.astro", "j.html",
        "k.h", "l.txt",
    ] {
        std::fs::write(dir.join(f), "x").unwrap();
    }
    let scan = list_scan_files(&dir, SCAN_EXTENSIONS);
    let parity = list_scan_files(&dir, PARITY_EXTENSIONS);
    drop(std::fs::remove_dir_all(&dir));
    assert_eq!(
        scan,
        [
            "a.rs", "b.rb", "c.ex", "d.exs", "e.swift", "f.cpp", "g.hpp", "h.gd", "i.astro"
        ]
    );
    assert_eq!(parity, ["a.rs"]);
}
