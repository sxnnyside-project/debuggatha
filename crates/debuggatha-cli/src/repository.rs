use debuggatha_engine::core::env::SystemEnv;
use debuggatha_engine::core::repository_intelligence::build_repository_context;

use crate::report::Format;
use crate::{Context, client_message};

/// Inspect what Debuggatha understands of the repository: stack, criteria, documentation.
pub(crate) fn run(context: &Context) -> Result<bool, String> {
    let root = context.root()?;
    let repo = build_repository_context(&root, &SystemEnv, None).map_err(|e| client_message(&e))?;
    match context.format() {
        Format::Json => println!(
            "{}",
            serde_json::to_string_pretty(&repo).unwrap_or_default()
        ),
        Format::Text => {
            let stack = &repo.stack;
            let signals = stack.languages.len()
                + stack.frameworks.len()
                + stack.build_systems.len()
                + stack.package_managers.len()
                + stack.runtimes.len();
            println!("Stack evidence: {signals} signals detected.");
            println!(
                "Dependencies: {} declared.",
                repo.dependencies.declared_versions.len()
            );
            println!("Criteria rules: {} active.", repo.criteria.rules.len());
            println!(
                "Documentation: {} sources indexed.",
                repo.documentation.sources.len()
            );
            println!(
                "Understanding: {} open questions.",
                repo.understanding.open_questions.len()
            );
            for question in &repo.understanding.open_questions {
                println!("  - {}", question.question);
            }
        }
    }
    Ok(false)
}
