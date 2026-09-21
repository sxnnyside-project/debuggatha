import type {
  CriteriaProfile,
  DependencyProfile,
  DocumentationProfile,
  OpenQuestion,
  StackProfile,
  UnderstandingConfidence,
  UnderstandingProfile,
} from "../types.js";

/**
 * Repository Understanding.
 *
 * The fallback skill — when deterministic discovery cannot confidently
 * explain the repository, this generates questions instead of guessing.
 * "Never invent missing information" (see CLAUDE.md): every question here
 * is derived from a concrete gap in what the other scanners found, never
 * from assumptions about what a "typical" project looks like.
 */

function confidenceFor(questionCount: number): UnderstandingConfidence {
  if (questionCount === 0) return "high";
  if (questionCount <= 2) return "medium";
  return "low";
}

export function deriveUnderstanding(
  stack: StackProfile,
  _dependencies: DependencyProfile,
  documentation: DocumentationProfile,
  criteria: CriteriaProfile,
): UnderstandingProfile {
  const openQuestions: OpenQuestion[] = [];

  if (stack.languages.length === 0) {
    openQuestions.push({
      question: "What language(s) does this project use?",
      reason:
        "No recognized manifest (Cargo.toml, package.json, pubspec.yaml, go.mod, Gemfile, pyproject.toml, build.gradle(.kts), pom.xml) was found at the repository root.",
    });
  }

  const isJsOrTs = stack.languages.some((signal) => signal.value === "JavaScript/TypeScript");
  if (isJsOrTs && stack.runtimes.length === 0) {
    openQuestions.push({
      question:
        "Which JavaScript/TypeScript runtime does this project target (Node.js, Bun, Deno)?",
      reason:
        "package.json is present but no runtime-specific marker (packageManager field, lockfile, engines.node, bunfig.toml, deno.json) was found.",
    });
  }

  if (documentation.sources.length === 0) {
    openQuestions.push({
      question: "What is this repository for?",
      reason: "No README or other documentation source was found.",
    });
  }

  if (criteria.rules.length === 0) {
    openQuestions.push({
      question: "Does this project have engineering conventions Debuggatha should follow?",
      reason:
        "No lint/format configuration and no rule-shaped documentation section (ARCHITECTURE.md, CONTRIBUTING.md, CLAUDE.md, *_CRITERIA.md) was found.",
    });
  }

  return {
    confidence: confidenceFor(openQuestions.length),
    openQuestions,
  };
}
