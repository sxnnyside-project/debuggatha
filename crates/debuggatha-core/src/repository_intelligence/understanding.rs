//! Repository Understanding: what is known and the questions a concrete gap raises.

use super::types::{
    Confidence, CriteriaProfile, DocumentationProfile, OpenQuestion, StackProfile,
    UnderstandingProfile,
};

fn confidence_for(question_count: usize) -> Confidence {
    match question_count {
        0 => Confidence::High,
        1 | 2 => Confidence::Medium,
        _ => Confidence::Low,
    }
}

/// Raises a question only when there is a concrete gap; it never invents an answer.
#[must_use]
pub fn derive_understanding(
    stack: &StackProfile,
    documentation: &DocumentationProfile,
    criteria: &CriteriaProfile,
) -> UnderstandingProfile {
    let mut open_questions = Vec::new();
    if stack.languages.is_empty() {
        open_questions.push(OpenQuestion {
            question: "What language(s) does this project use?".to_owned(),
            reason: "No recognized manifest (Cargo.toml, package.json, pubspec.yaml, go.mod, Gemfile, pyproject.toml, build.gradle(.kts), pom.xml, Package.swift, mix.exs, CMakeLists.txt, *.csproj, *.sln) was found at the repository root.".to_owned(),
        });
    }
    let is_js_or_ts = stack
        .languages
        .iter()
        .any(|s| s.value == "JavaScript/TypeScript");
    if is_js_or_ts && stack.runtimes.is_empty() {
        open_questions.push(OpenQuestion {
            question: "Which JavaScript/TypeScript runtime does this project target (Node.js, Bun, Deno)?".to_owned(),
            reason: "package.json is present but no runtime-specific marker (packageManager field, lockfile, engines.node, bunfig.toml, deno.json) was found.".to_owned(),
        });
    }
    if documentation.sources.is_empty() {
        open_questions.push(OpenQuestion {
            question: "What is this repository for?".to_owned(),
            reason: "No README or other documentation source was found.".to_owned(),
        });
    }
    if criteria.rules.is_empty() {
        open_questions.push(OpenQuestion {
            question: "Does this project have engineering conventions Debuggatha should follow?".to_owned(),
            reason: "No lint/format configuration and no rule-shaped documentation section (ARCHITECTURE.md, CONTRIBUTING.md, CLAUDE.md, *_CRITERIA.md) was found.".to_owned(),
        });
    }
    UnderstandingProfile {
        confidence: confidence_for(open_questions.len()),
        open_questions,
    }
}
