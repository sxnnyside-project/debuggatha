//! What the TypeScript CLI and MCP server took, and hosts still pass: `-C`, `--cwd`, `--json`
//! and the `DEBUGGATHA_*` variables.

#![allow(clippy::expect_used, clippy::unwrap_used)] // a failed expect is a failed test

use std::process::{Command, Stdio};

use serde_json::Value;

fn debuggatha() -> Command {
    Command::new(env!("CARGO_BIN_EXE_debuggatha"))
}

#[test]
fn json_and_cwd_are_the_old_spellings_of_format_and_root() {
    let dir = std::env::temp_dir().join(format!("debuggatha-compat-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("a temporary directory");
    for place in ["-C", "--cwd"] {
        let out = debuggatha()
            .args([place, dir.to_str().unwrap_or("."), "packs", "--json"])
            .output()
            .expect("the binary runs");
        assert!(out.status.success(), "{place}: {out:?}");
        assert!(
            serde_json::from_slice::<Value>(&out.stdout).is_ok(),
            "{place}"
        );
    }
    std::fs::remove_dir_all(&dir).expect("the temporary directory goes");
}

#[test]
fn the_variables_a_host_sets_are_read_and_a_wrong_one_is_refused() {
    let ok = debuggatha()
        .arg("serve")
        .stdin(Stdio::null())
        .env("DEBUGGATHA_ANALYZERS", "auto")
        .env("DEBUGGATHA_SEMANTIC", "off")
        .output()
        .expect("the binary runs");
    assert!(ok.status.success(), "{ok:?}");
    let bad = debuggatha()
        .arg("serve")
        .stdin(Stdio::null())
        .env("DEBUGGATHA_SEMANTIC", "bogus")
        .output()
        .expect("the binary runs");
    assert!(!bad.status.success());
}
