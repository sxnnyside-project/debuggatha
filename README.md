# Debuggatha

![Version](https://img.shields.io/badge/version-0.1.0-blue)
![License](https://img.shields.io/badge/License-MIT-green)
[![CI](https://github.com/sxnnyside-project/debuggatha/workflows/CI/badge.svg)](https://github.com/sxnnyside-project/debuggatha/actions)

<p align="center">
  <strong>Contextual ✦ Deterministic ✦ Extensible</strong><br>
  <em>A senior code reviewer, not a chat wrapper.</em>
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

**Debuggatha** acts as a dedicated engineering reviewer that points out architectural flaws, security issues, and logical errors in your code.

It exists to move beyond chat-wrapper AI reviews. Debuggatha understands a repository's context — stack, architecture, and its own stated rules — before it reviews anything, ensuring every finding it produces traces back to a real source.

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

### Prerequisites

- Bun (latest)
- Node.js (>= 20)

### From Source

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
