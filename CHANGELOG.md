# Changelog

All notable changes to **Debuggatha** are documented here.

This project follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)
and [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [Unreleased]

---

## [0.2.0] — 2026-09-21

### Added

- Review Skills for files, diffs, and architecture over one shared detector engine, with Review Packs for many ecosystems and Criteria Resolution against the repository's own stack, conventions, and documentation.
- Findings carry the column, a rubric-based severity and confidence, why the pattern matters, a concrete fix, the code before and after, and, for some rules, an exact edit marked `safe` or `review`.
- Code-aware detectors: patterns inside strings and comments do not match, and secret detection flags known credential formats and high-entropy values assigned to secret-named variables.
- Reviews skip code the project does not own: dependencies, build output, generated and minified files, and anything `.gitignore` excludes.
- Findings Ledger in `.debuggatha/ledger.json` with a five-state lifecycle, repository-relative locations, and atomic writes shared by the CLI, MCP server, and editor.
- Baseline (`baseline create|show|clear`, MCP `create_baseline`) so later reviews report only what is new, and `--include-baselined` to see everything.
- Inline suppression with `debuggatha-ignore`, `-next-line`, and `-file` comments and a required reason, and Repository Memory for accepted deviations and exceptions in `.debuggatha/memory.json`.
- External analyzers run as separate processes: gitleaks, Biome, ESLint, Ruff, ktlint, detekt, PHPStan, Clippy, and OSV-Scanner. Each finding is labeled with the tool, version, and license; tools that run project code or use the network run only when enabled with `--analyzer` or `DEBUGGATHA_ANALYZERS`.
- `debuggatha analyzers` and the MCP `list_analyzers` tool, and a license gate (`just licenses`) that runs in `just check` and CI.
- Optional semantic pass (`--semantic ollama|lmstudio`, MCP `semantic: true`): a local model raises suspicions labeled with the model, at low confidence, kept only when they quote the line they are about. Code goes only to `localhost` or the connected client's own model through sampling. `just semantic-eval` measures a model on a labeled set.
- CLI `review` options `--all`, `--changed-since`, `--base`, `--diff -`, `--fail-on`, `--fail-on-new`, `--output`, and `--no-persist`, with formats `text`, `json`, `sarif`, `github`, `markdown`, and `gitlab`.
- `.debuggatha/config.json` with `review` and `analyzers` defaults and monorepo `ignore` and per-path `overrides`; a config can make a review stricter or quieter but cannot enable an analyzer that runs project code or uses the network, or the semantic pass.
- `debuggatha init` (config, GitHub Actions or GitLab pipeline, optional pre-commit hook), `trends`, `doctor`, `packs`, `policies`, `memory`, `findings`, `repository`, and `completion` commands, and standalone executables (`just binaries`).
- MCP server with titled, annotated tools that have output schemas, stdio and Streamable HTTP transports, workspace roots, and sampling; `review_changes` reports what a change introduced, fixed, and reopened, alongside `explain_finding`, `suppress_finding`, and the `review-flow` and `fix-findings` prompts. An MCP Registry manifest and `just smoke` check the packed CLI and MCP under Node and Bun.
- VS Code extension with diagnostics, a findings view, hover, quick fixes, a status bar item, reviews in their own cancellable process, and the settings `reviewDepth`, `reviewOnSave`, `minimumSeverity`, `logLevel`, `extraReviewPacks`, and `enabledAnalyzers`. In an untrusted workspace only built-in detectors run, and analyzer and local-model settings are machine scope.
- Integration guide for coding agents (`docs/INTEGRATIONS.md`), `just install-cli`, and a launch configuration to run the extension from source.

### Security

- Git refs are validated and passed to git without a shell.
- Secrets are redacted before they reach reports, the ledger, or a model.
- Analyzers that run project code or use the network, and the semantic pass, can be enabled only by the person running the review.

---

## [0.1.0] — 2026-07-10

### Added
- Initial public preview release of the completely rewritten Debuggatha engine.
- Complete domain modeling for static review, capability registries, and persistent ledgers.

---

[Unreleased]: https://github.com/sxnnyside-project/debuggatha/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/sxnnyside-project/debuggatha/releases/tag/v0.2.0
[0.1.0]: https://github.com/sxnnyside-project/debuggatha/releases/tag/v0.1.0
