# Changelog

All notable changes to **Debuggatha** are documented here.

This project follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)
and [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [Unreleased]

---

## [0.3.0] — 2026-11-19

The engine is rewritten in Rust. Findings, the ledger fingerprint and the CLI and MCP surface keep their shape; every intended difference from the TypeScript engine is listed in [docs/PARITY.md](docs/PARITY.md).

### Added

- A native Rust workspace: `debuggatha-core` (the review domain), `debuggatha-engine` (runs reviews) and `debuggatha-cli`. The `debuggatha` binary is the CLI, the daemon and the MCP server.
- A syntax layer: oxc for JavaScript and TypeScript, tree-sitter for C#, C++, Dart, Elixir, GDScript, Go, HTML, Java, Kotlin, PHP, Python, Ruby, Rust and Swift, and Vue single-file components. Every file gets a parse status.
- Detectors that read a language-neutral semantic model instead of lines of text: calls, promises, resource leaks, taint, environment variables, secrets, modules and functions, with per-language concept tables in `data/queries`.
- 43 Review Packs as TOML data validated against a schema (the TypeScript engine had 29), and `docs/PACK_SPEC.md`.
- The Analysis stage in Rust: module structure, dependency graph, layers and cycles, public API surface, ownership, change impact and ecosystem detection.
- `coverage` in every review: what could not be checked (a language with no parser, a rule with no detector) is stated, and "0 findings" is never reported as clean. The 0.2.0 reports had no such field.
- `debuggatha serve`, the daemon of Protocol v1 (NDJSON over stdio, bound to one repository root), documented in [docs/PROTOCOL.md](docs/PROTOCOL.md).
- MCP: for hosts that forbid server-to-client requests, a tool that needs the client's model answers `input_required` and the client continues with `review/resume`.
- Fifteen JSON Schemas of the contracts in `docs/schemas/`, generated from the Rust types and checked by tests; the extension's types are generated from them (`just vscode-contracts`).
- A corpus: 61 real repositories pinned in `corpus/manifest.json`, goldens the TypeScript engine wrote, and parity tests that compare the Rust engine with them; measured false-positive rates in [docs/DETECTOR_QUALITY.md](docs/DETECTOR_QUALITY.md).
- Distribution: native binaries for macOS arm64, Linux x64 and arm64 and Windows x64; one npm package per platform (`@sxnnyside/debuggatha-<os>-<cpu>`); `@sxnnyside/debuggatha-cli` and `@sxnnyside/debuggatha-mcp` as launchers that carry no engine; the crates `debuggatha-core`, `debuggatha-engine` and `debuggatha-cli`.
- A release workflow: version check against every manifest, the full gate, a build per platform, publication to npm (trusted publishing with provenance), crates.io, the VS Code Marketplace and Open VSX, and a GitHub release whose notes come from Conventional Commits. `scripts/` holds the version check, packaging and publish scripts.
- Quality gates: `rust-toolchain.toml` pinned, clippy pedantic with `-D warnings`, `cargo deny`, and `just docs` (rustdoc as errors). Every public item of core and engine is documented, and the style guides are in `docs/styleguide/`.
- VS Code: the `debuggatha.cliPath` setting (user scope only, so a repository cannot choose the program the extension runs), and discovery of the binary in `~/.cargo/bin`, Homebrew's and `/usr/local/bin` when the editor's `PATH` lacks it.
- Community files from the Sxnnyside bundle: `SECURITY.md`, `SUPPORT.md`, issue and pull request templates, funding file, banner, and `ARCHITECTURE.md` at the root.

### Changed

- The VS Code extension is a client of the daemon and no longer bundles an engine (a 31 KB bundle). Its toolchain moves to TypeScript 7, Vitest 5, Vite 8, esbuild 0.28, `vsce` 4 and Mocha 11.
- The MCP prompts `review-flow` and `fix-findings` ask for the smallest change that fixes a finding, and for what the review did not check.
- The license gate that was a Node script (`just licenses`) is now `cargo deny`, part of `just check` and CI.
- A comment between `#[cfg(test)]` and its module no longer makes test code count as production code in `no-unwrap-expect`.
- The documentation follows the Sxnnyside templates; `docs/ARCHITECTURE.md` is now `ARCHITECTURE.md`, and the glossary, ADRs, protocol, parity and quality documents were rewritten for the Rust engine.

### Removed

- The TypeScript engine and its `packages/` workspace (`core`, `engine`, `packs`, `cli`, `mcp`), the Bun and Turborepo tooling, and the `@debuggatha/*` workspace packages. The last commit that has them is the tag `ts-engine-final`.
- `DEBUGGATHA_LOG_LEVEL`, `DEBUGGATHA_CACHE_ENABLED`, `DEBUGGATHA_ANALYZER_TIMEOUT` and the `--verbose` flag: the binary writes nothing to stderr beyond errors, and there is no context cache to switch off.

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
- The CLI and MCP server publish to npm as `@sxnnyside/debuggatha-cli` and `@sxnnyside/debuggatha-mcp`.
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

[Unreleased]: https://github.com/sxnnyside-project/debuggatha/compare/v0.3.0...HEAD
[0.3.0]: https://github.com/sxnnyside-project/debuggatha/releases/tag/v0.3.0
[0.2.0]: https://github.com/sxnnyside-project/debuggatha/releases/tag/v0.2.0
[0.1.0]: https://github.com/sxnnyside-project/debuggatha/releases/tag/v0.1.0
