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

**Debuggatha** is a senior code reviewer that runs as an MCP server, a CLI, and a VS Code extension. You call it after Claude Code, Copilot, or Cursor writes code — it reads the diff, checks it against your repository's own stack, conventions, and documented rules, and hands back findings that cite exactly why each one exists.

**How this is different from Copilot, Cursor, or Claude Code**: those tools write and edit code — they're built to generate. Debuggatha never writes a line of your code; it has one job, review, and it does that job by first understanding your repository (stack detection, config/lint conventions, ADRs, README intent) instead of pattern-matching from general training data. Every finding cites a source — a rule in a Review Pack, a convention your own repo declares, a recognized standard like OWASP, or evidence in the code itself. No source, no finding — "I think this looks off" never ships. Findings also persist across sessions in a versioned ledger with a real lifecycle (open → acknowledged → resolved → dismissed → reopened), so a review isn't a wall of text you re-read from scratch every time — it's a project you can track.

**Why install it**: if you're already using an AI pair programmer, you already have a fast generator and no dedicated reviewer. Debuggatha is the second opinion that has actually read your codebase — plug it into the same MCP host you're already using (Claude Code, Claude Desktop, Cursor, Copilot agent mode) and it starts citing real findings against your real rules, not generic best-practice trivia.

Debuggatha is a modular platform built on top of four core pillars: Repository Intelligence, the Review Engine, the Knowledge System, and the Findings Ledger.

### Philosophy

> *"A senior code reviewer, not a chat wrapper."*

This is a Sxnnyside Project.

## Features

- **Repository Intelligence**: Automatically detects the stack, dependencies, and local documentation to understand the environment.
- **Review Engine**: Manages the review domain model, ensuring findings are strongly typed, located, and substantiated by evidence.
- **Knowledge System**: Manages Review Packs and Rules to dynamically assemble the context the agent uses to review code.
- **Findings Ledger**: An immutable, version-controlled history that tracks findings across review sessions with a strict 5-state lifecycle.

## Installation

### Use it

- **VS Code / Cursor**: install the [Debuggatha extension](https://marketplace.visualstudio.com/items?itemName=SxnnysideProject.debuggatha) from the Marketplace, or the [Open VSX](https://open-vsx.org/extension/SxnnysideProject/debuggatha) listing. See [apps/vscode/README.md](apps/vscode/README.md).
- **MCP host** (Claude Code, Claude Desktop, Copilot agent mode): point your MCP config at `@debuggatha/mcp`. See [packages/mcp/README.md](packages/mcp/README.md).
- **Terminal / CI**: `bunx @debuggatha/cli review` — no editor required. See [packages/cli/README.md](packages/cli/README.md).

### Build from source

**Prerequisites**

- Bun (latest)
- Node.js (>= 20)

```bash
git clone https://github.com/sxnnyside-project/debuggatha.git
cd debuggatha

bun install
bun run build
```

## Usage

```bash
# Check formatting and typing
bun run check
bun run typecheck

# Run unit tests across packages
bun run test
```

For the CLI and VS Code Client specifics, refer to their respective app-level instructions.

## Architecture

```
debuggatha/
├── apps/         # VS Code client and docs site
├── packages/     # The core engines, intelligence, ledger, and knowledge systems
└── docs/         # Architecture notes, ADRs, and the Review Pack spec
```

For a detailed breakdown, see [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

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
