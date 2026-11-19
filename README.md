# Debuggatha

![Debuggatha banner](docs/assets/debuggatha-banner.png)

![Version](https://img.shields.io/badge/version-0.3.0-blue)
![License](https://img.shields.io/badge/License-MIT-green)
[![CI](https://github.com/sxnnyside-project/debuggatha/actions/workflows/ci.yml/badge.svg)](https://github.com/sxnnyside-project/debuggatha/actions/workflows/ci.yml)

<p align="center">
  <strong>Contextual ✦ Deterministic ✦ Extensible</strong><br>
  <em>The engineering reviewer that remembers — every finding traced to a real rule, tracked from open to resolved, judged against the repo you actually wrote.</em>
</p>

<p align="center">
  <a href="#about">About</a> ✦
  <a href="#features">Features</a> ✦
  <a href="#installation">Installation</a> ✦
  <a href="#usage">Usage</a> ✦
  <a href="#architecture">Architecture</a> ✦
  <a href="#contributing">Contributing</a>
</p>

---

## About

**Debuggatha** is a senior code reviewer that runs as an MCP server, a CLI, and a VS Code extension. It reads a diff, a file, or a repository against the repository's own stack, conventions, and documented rules, and returns findings that cite exactly why each one exists.

Copilot, Cursor, and Claude Code write code; Debuggatha never writes a line of yours. It has one job, review, and it does it by first understanding the repository (stack, lint and config conventions, ADRs, README intent) instead of pattern-matching from general training data. Every finding cites a source: a rule in a Review Pack, a convention the repository declares, a recognized standard such as OWASP, the output of an installed analyzer, or evidence in the code itself. No source, no finding.

Findings persist in a versioned ledger with a real lifecycle (open, acknowledged, resolved, dismissed, reopened), so a review is a project you can track. The deterministic core does not depend on any model; a local model is an optional extra that can raise suspicions but never closes, hides, or lowers a finding.

### Philosophy

> *"A senior code reviewer, not a chat wrapper."*

Part of the Sxnnyside Project ecosystem.

## Features

- **Repository-aware**: detects the stack, dependencies, conventions, and documentation, and resolves the rules that apply.
- **Evidence for every finding**: a rubric-based severity and confidence, the column, why it matters, a concrete fix, and the code before and after.
- **Review Packs**: rule content for TypeScript, JavaScript, React, Vue, Svelte, Node.js, Bun, Electron, Tauri, Go, Rust, Kotlin, PHP, Dart, OWASP, performance, accessibility, and more.
- **External analyzers**: gitleaks, Biome, ESLint, Ruff, ktlint, detekt, PHPStan, Clippy, and OSV-Scanner run as separate processes; tools that run project code or use the network run only when you enable them.
- **Findings Ledger**: `.debuggatha/ledger.json` tracks findings across sessions and clients, with baselines for adopting an existing repository.
- **Suppression and memory**: inline `debuggatha-ignore` comments and repository memory accept a finding durably, with a required reason.
- **CI ready**: `--changed-since`, `--fail-on-new`, SARIF, GitHub, GitLab, and Markdown reports; `debuggatha init` writes a config and a pipeline.
- **Three surfaces, one engine**: the Rust binary is the CLI, the daemon, and the MCP server; the VS Code extension is its client. They share one pipeline and one ledger.

## Installation

### Prerequisites

- [Rust](https://rustup.rs) (the toolchain is pinned in `rust-toolchain.toml`) and [cargo-deny](https://github.com/EmbarkStudios/cargo-deny): the engine, CLI, daemon, and MCP server
- [just](https://github.com/casey/just): command surface for the whole repository
- [Node.js](https://nodejs.org) (see `apps/vscode/.node-version`) and [pnpm](https://pnpm.io): only for the VS Code extension (`just vscode-install`)

### From Source

```bash
git clone https://github.com/sxnnyside-project/debuggatha.git
cd debuggatha

just install       # fetch the crates
just build         # target/release/debuggatha
just install-cli   # put `debuggatha` on your PATH
just check         # the full gate: native, the extension, the npm packages
```

`just --list` shows every recipe. The unprefixed ones drive the Rust workspace, and `vscode-*` the extension.

### From a Registry

- **VS Code / Cursor**: install the [Debuggatha extension](https://marketplace.visualstudio.com/items?itemName=SxnnysideProject.debuggatha) from the Marketplace, or from [Open VSX](https://open-vsx.org/extension/SxnnysideProject/debuggatha). See [apps/vscode/README.md](apps/vscode/README.md).
- **MCP host** (Claude Code, Claude Desktop, Cursor): `claude mcp add debuggatha -- npx -y @sxnnyside/debuggatha-mcp` (`bunx` works too). The package launches the native binary (`DEBUGGATHA_CLI`, the package of your platform, or `PATH`). See [npm/mcp/README.md](npm/mcp/README.md).
- **Terminal / CI**: `npx @sxnnyside/debuggatha-cli review` (or `bunx`), `cargo install debuggatha-cli`, or the archive of your platform from the releases page (with `SHA256SUMS`). See [npm/cli/README.md](npm/cli/README.md).

## Usage

```bash
debuggatha review                        # review what changed since the last commit
debuggatha review --all                  # review the whole repository
debuggatha baseline create               # adopt: report only what is new from here on
debuggatha init                          # write a config and a CI workflow
debuggatha doctor                        # diagnose the setup
```

To connect a coding agent (Claude Code, Qwen Code, Cursor, and others), see [docs/INTEGRATIONS.md](docs/INTEGRATIONS.md). For every command and flag, run `debuggatha --help`.

## Architecture

For a detailed breakdown, see [ARCHITECTURE.md](ARCHITECTURE.md).

## Contributing

Contributions are accepted. See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

Before contributing, read the [Code of Conduct](CODE_OF_CONDUCT.md).

## License

This project is licensed under the MIT License — see the [LICENSE](LICENSE) file for details.

---

<p align="center">
  <strong>Debuggatha</strong> — A Sxnnyside Project<br>
  <em>&copy; 2026 Sxnnyside Project</em>
</p>
