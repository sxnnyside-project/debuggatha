//! OSV-Scanner: known vulnerabilities in the dependencies.

use crate::analyzers::sarif::{SarifOptions, parse_sarif};
use crate::analyzers::types::{Adapter, AnalyzerFinding, Invocation, RepoFacts, Trust};
use std::path::Path;

fn osv_applies(facts: &RepoFacts) -> bool {
    facts.has_file_named(&[
        "package-lock.json",
        "yarn.lock",
        "pnpm-lock.yaml",
        "bun.lock",
        "Cargo.lock",
        "composer.lock",
        "go.sum",
        "Gemfile.lock",
        "poetry.lock",
        "uv.lock",
        "requirements.txt",
        "gradle.lockfile",
        "pom.xml",
    ])
}

fn osv_parse(stdout: &str, root: &Path) -> Result<Vec<AnalyzerFinding>, String> {
    if stdout.trim().is_empty() {
        return Ok(Vec::new());
    }
    parse_sarif(
        stdout,
        root,
        &SarifOptions {
            category: |_| "security",
            severity: None,
            sensitive: false,
        },
    )
}

pub static OSV_SCANNER: Adapter = Adapter {
    id: "osv-scanner",
    name: "OSV-Scanner",
    license: "Apache-2.0",
    homepage: "https://google.github.io/osv-scanner",
    covers: "known vulnerabilities in dependencies",
    // It sends package names and versions to the OSV.dev API.
    trust: Trust::Network,
    applies: osv_applies,
    local_binaries: &[],
    binary_names: &["osv-scanner"],
    prefix: &[],
    version_args: &["--version"],
    command: |_| Invocation {
        args: ["scan", "source", "--format", "sarif", "--recursive", "."]
            .map(str::to_owned)
            .to_vec(),
        cleanup: None,
    },
    // 1: vulnerabilities found. 128: nothing to scan.
    ok_exit_codes: &[0, 1, 128],
    parse: osv_parse,
    supersedes: &[],
    equivalent_to: &[],
    file_filter: None,
};
