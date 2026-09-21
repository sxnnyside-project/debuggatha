# Changelog

All notable changes to **Debuggatha** are documented here.

This project follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)
and [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [Unreleased]

### Added

**Review**
- Review Skills for files, diffs, and architecture, over a shared detector engine; Review Packs for many ecosystems; Criteria Resolution against the repository's own stack, conventions, and documentation.
- Findings carry the column, a rubric-based severity and confidence, why the pattern matters, a concrete fix, and the code before and after. Some carry an exact edit marked `safe` or `review`.
- Detectors read code, not text: strings and comments do not match. Secret detection flags known credential formats and high-entropy values assigned to secret-named variables; a found secret is redacted in reports and in the ledger.
- Reviews skip code the project does not own: dependencies, build output, generated and minified files, and anything `.gitignore` excludes.
- `review --all`, `--changed-since <ref>`, `--base`, and `--diff -`; `--fail-on-new` fails only on findings on lines a change added.

**Ledger, baseline, and suppression**
- Findings Ledger in `.debuggatha/ledger.json`: a lifecycle (open, acknowledged, resolved, dismissed, reopened), repository-relative locations, and atomic writes shared by the CLI, MCP server, and editor.
- Baseline (`baseline create|show|clear`, MCP `create_baseline`): later reviews report and fail on only what is new; `--include-baselined` shows everything.
- Inline suppression with `debuggatha-ignore`, `-next-line`, and `-file`, with rule ids and a reason; reviews report what a comment hid.
- Repository Memory: accepted deviations and exceptions with a required reason, kept in `.debuggatha/memory.json`.
- `debuggatha trends`: how findings are moving, from the ledger's history.

**External analyzers**
- gitleaks, Biome, ESLint, Ruff, ktlint, detekt, PHPStan, Clippy, and OSV-Scanner run as separate processes and add findings labeled with the tool, version, and license. Tools that only read source run by default; those that run project code or use the network run only when enabled with `--analyzer <id>` or `DEBUGGATHA_ANALYZERS`, never by a repository file or a tool call.
- A missing, failing, or hung tool is reported and never fails a review; its earlier findings are not called fixed while it did not run. A linter's finding replaces a built-in detector's for the same rule.
- `debuggatha analyzers` and MCP `list_analyzers`; `--analyzer`, `--no-analyzers`, `--analyzer-timeout`.
- A license gate (`just licenses`, part of `just check`): everything that ships is permissively licensed and no published package depends on or contains an analyzer.

**Semantic pass (optional)**
- `--semantic ollama|lmstudio` and MCP `semantic: true` have a local model read changed code. Claims are labeled suspicions at low confidence, kept only when they quote the line they are about, and never close, hide, or lower a finding. Code goes only to `localhost` or the connected client's own model; secret lines and secret files are never sent. Verification is off by default.
- `just semantic-eval` measures a model on a labeled set and reports precision and recall per category against benchmark marks.

**CLI**
- `review` formats `text`, `json`, `sarif`, `github`, `markdown` (a pull-request comment), and `gitlab` (Code Quality); `--fail-on`, `--output`, `--no-persist`.
- `.debuggatha/config.json` with `review` and `analyzers` defaults, monorepo `ignore` and per-path `overrides`; the nearest config at or above the working directory applies. A config can make a review stricter or quieter but cannot enable an analyzer that runs project code or uses the network, or the semantic pass.
- `debuggatha init` writes a config and a GitHub Actions or GitLab pipeline, and with `--hooks` a pre-commit hook; it never overwrites a file it did not write and recognizes monorepos.
- `doctor`, `packs`, `policies`, `memory`, `findings`, `repository`, and `completion`; standalone executables (`just binaries`).

**MCP server**
- Tools with titles, annotations, and output schemas; stdio and Streamable HTTP; workspace `roots` as the default repository; sampling for the semantic pass.
- The agent loop: `review_changes` reports what a change introduced, fixed, and reopened; `explain_finding` and `suppress_finding` close it. Prompts `review-flow` and `fix-findings`, and server instructions that teach the loop.
- A registry manifest (`packages/mcp/server.json`) and `just smoke`, which installs the packed tarballs into an empty project and runs the installed binaries under Node and Bun.

**VS Code extension**
- Diagnostics, a findings view, and a status bar item; hover with the rule, why it matters, and the fix; quick fixes that apply the engine's exact edit and accept a finding in code with a reason.
- Reviews run in their own process and can be cancelled; the extension host is never blocked.
- Settings `reviewDepth` (`quick`, `full`, `architectural`), `reviewOnSave`, `minimumSeverity`, `logLevel`, `extraReviewPacks`, and `enabledAnalyzers`; commands to review the file, selection, or workspace, cancel a review, and show the log.
- In a workspace you have not trusted only built-in detectors run; analyzer and local-model settings are machine scope, so a repository cannot switch them on.
- Integration tests run in the oldest supported VS Code (1.90.2) and the current stable.

**Tooling**
- Turborepo and Bun workspaces (`core`, `engine`, `packs`, `cli`, `mcp`) beside the extension (Node, pnpm, esbuild); a root `Justfile`; CI runs `just check`.

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

[Unreleased]: https://github.com/sxnnyside-project/debuggatha/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/sxnnyside-project/debuggatha/releases/tag/v0.1.0
