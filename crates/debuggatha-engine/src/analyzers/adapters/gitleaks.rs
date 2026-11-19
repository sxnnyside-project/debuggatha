//! gitleaks: secrets in the working tree.

use crate::analyzers::sarif::{SarifOptions, parse_sarif};
use crate::analyzers::types::{Adapter, AnalyzerFinding, Invocation, RunContext, Trust};
use crate::scan_scope::IGNORED_DIRS;
use debuggatha_core::review_engine::Severity;
use std::path::Path;
use std::sync::atomic::{AtomicU32, Ordering};

/// Makes the name of a generated config directory unique within the process.
static COUNTER: AtomicU32 = AtomicU32::new(0);

fn gitleaks_command(context: &RunContext<'_>) -> Invocation {
    let mut args: Vec<String> = [
        "dir",
        ".",
        "--no-banner",
        "--log-level",
        "error",
        "--redact",
        "--report-format",
        "sarif",
        "--report-path",
        "-",
        "--exit-code",
        "0",
    ]
    .map(str::to_owned)
    .to_vec();
    if [".gitleaks.toml", "gitleaks.toml"]
        .iter()
        .any(|name| context.root.join(name).exists())
    {
        return Invocation {
            args,
            cleanup: None,
        };
    }
    // No repository config: scan what the project owns. `dir` does not read `.gitignore`.
    let directory = std::env::temp_dir().join(format!(
        "debuggatha-gitleaks-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    let skipped: Vec<String> = IGNORED_DIRS.iter().map(|d| escape_regex(d)).collect();
    let config = directory.join("gitleaks.toml");
    let text = format!(
        "[extend]\nuseDefault = true\n\n[[allowlists]]\npaths = ['''(^|/)({})/''']\n",
        skipped.join("|")
    );
    if std::fs::create_dir_all(&directory)
        .and_then(|()| std::fs::write(&config, text))
        .is_ok()
    {
        args.extend(["--config".to_owned(), config.to_string_lossy().into_owned()]);
        return Invocation {
            args,
            cleanup: Some(directory),
        };
    }
    Invocation {
        args,
        cleanup: None,
    }
}

fn escape_regex(text: &str) -> String {
    let mut out = String::new();
    for c in text.chars() {
        if ".\\+*?()|[]{}^$".contains(c) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

fn gitleaks_parse(stdout: &str, root: &Path) -> Result<Vec<AnalyzerFinding>, String> {
    parse_sarif(
        stdout,
        root,
        &SarifOptions {
            category: |_| "security",
            severity: Some(|_| Severity::Critical),
            sensitive: true,
        },
    )
}

/// gitleaks, which looks for secrets in the working tree.
pub static GITLEAKS: Adapter = Adapter {
    id: "gitleaks",
    name: "Gitleaks",
    license: "MIT",
    homepage: "https://github.com/gitleaks/gitleaks",
    covers: "secrets in any language",
    trust: Trust::Safe,
    applies: |_| true,
    local_binaries: &[],
    binary_names: &["gitleaks"],
    prefix: &[],
    version_args: &["version"],
    command: gitleaks_command,
    ok_exit_codes: &[0],
    parse: gitleaks_parse,
    supersedes: &["no-hardcoded-secrets"],
    equivalent_to: &[],
    file_filter: None,
};
