# Debuggatha

![Version](https://img.shields.io/badge/version-0.1.0-blue)
![License](https://img.shields.io/badge/License-MIT-green)
[![CI](https://github.com/sxnnyside-project/debuggatha/workflows/CI/badge.svg)](https://github.com/sxnnyside-project/debuggatha/actions)

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

**Debuggatha** is a senior code reviewer that runs as an MCP server, a CLI, and a VS Code extension. Point it at a diff, a file, or a repository; it reads the code against your repository's own stack, conventions, and documented rules, and returns findings that cite exactly why each one exists.

It never writes a line of your code. It has one job, review, and it does that job by first understanding the repository (stack detection, lint and config conventions, ADRs, README intent) instead of pattern-matching from general training data. Every finding cites a source: a rule in a Review Pack, a convention the repository declares, a recognized standard such as OWASP, the output of an installed analyzer, or evidence in the code itself. No source, no finding.

Findings persist in a versioned ledger with a real lifecycle (open, acknowledged, resolved, dismissed, reopened), so a review is a project you can track rather than a wall of text you re-read.

The deterministic core does not depend on any model. A language model is an optional extra, kept on a short leash (see [Semantic pass](#semantic-pass)).

### Philosophy

> *"A senior code reviewer, not a chat wrapper."*

## Features

- **Repository-aware**: detects the stack, dependencies, conventions, and local documentation, and resolves the rules that apply to this repository.
- **Evidence for every finding**: severity and confidence follow a documented rubric; findings carry the column, why it matters, a concrete fix, and the same code before and after. Some carry an exact edit marked `safe` or `review`.
- **Review Packs**: rule content for TypeScript, JavaScript, React, Vue, Svelte, Node.js, Bun, Express, Electron, Tauri, Go, Rust, Kotlin, PHP, Dart, ASP.NET Core, OWASP, performance, accessibility, and more.
- **External analyzers**: gitleaks, Biome, ESLint, Ruff, ktlint, detekt, PHPStan, Clippy, and OSV-Scanner run as separate processes and land in the same ledger, labeled with tool, version, and license. Tools that execute project code or use the network run only when you enable them.
- **Findings Ledger**: `.debuggatha/ledger.json` tracks findings across sessions and clients; baselines let a repository adopt Debuggatha and fail only on what is new.
- **Suppression and memory**: inline `debuggatha-ignore` comments and repository memory accept a finding durably, with a required reason.
- **CI ready**: `--changed-since`, `--fail-on-new`, and `--format text|json|sarif|github|markdown|gitlab`; `debuggatha init` writes a config and a GitHub Actions or GitLab pipeline.
- **Monorepo aware**: `ignore` and per-path `overrides` in `.debuggatha/config.json`.
- **Three surfaces, one engine**: MCP server, CLI, and VS Code extension share the same pipeline and the same ledger.

### Semantic pass

An optional pass has a local model (Ollama or LM Studio, or the MCP client's own model through sampling) read the changed code for defects no analyzer sees. Its claims are labeled suspicions at low confidence, are kept only when they quote the line they are about, and never close, hide, or lower a finding. Code goes only to `localhost`; secrets never leave. It is off unless you ask for it.

## Installation

### Prerequisites

- [Bun](https://bun.sh) (see `.bun-version`): packages, CLI, and MCP server
- [Node.js](https://nodejs.org) (see `apps/vscode/.node-version`) and [pnpm](https://pnpm.io): VS Code extension only
- [just](https://github.com/casey/just): command surface for the whole repository

### From Source

```bash
git clone https://github.com/sxnnyside-project/debuggatha.git
cd debuggatha

just install
just build
```

### From a Registry

- **VS Code / Cursor**: install the [Debuggatha extension](https://marketplace.visualstudio.com/items?itemName=SxnnysideProject.debuggatha) from the Marketplace, or from [Open VSX](https://open-vsx.org/extension/SxnnysideProject/debuggatha). See [apps/vscode/README.md](apps/vscode/README.md).
- **MCP host** (Claude Code, Claude Desktop, Cursor): `claude mcp add debuggatha -- bunx @debuggatha/mcp` (`npx -y` works too). See [packages/mcp/README.md](packages/mcp/README.md).
- **Terminal / CI**: `bunx @debuggatha/cli review` (or `npx`), or a standalone executable from the releases page. See [packages/cli/README.md](packages/cli/README.md).

## Usage

```bash
debuggatha review                          # review what changed since the last commit
debuggatha review --all                    # review the whole repository
debuggatha review --changed-since main --fail-on-new --fail-on high
debuggatha baseline create                 # adopt: report only what is new from here on
debuggatha init                            # write a config and a CI workflow
debuggatha analyzers                       # what is installed and what a review would run
debuggatha doctor                          # diagnose the setup
```

Working on the repository itself:

```bash
just            # list every recipe
just dev        # rebuild packages and watch the extension
just check      # format check, lint, typecheck, test, build, license gate: what CI runs
just smoke      # install the packed CLI and MCP into an empty project and run them
just format     # apply formatting and import organization
just package    # build the .vsix
```

## Architecture

```
debuggatha/
├── apps/         # The VS Code extension (Node + pnpm)
├── packages/     # core, packs, and engine, plus the published cli and mcp
├── docs/         # Architecture notes, ADRs, and the Review Pack spec
└── scripts/      # License gate and release tooling
```

`core` is the review domain, `engine` runs reviews, `packs` holds rule content, and the three adapters (`cli`, `mcp`, the extension) import only `@debuggatha/engine`. For a detailed breakdown, see [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

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
