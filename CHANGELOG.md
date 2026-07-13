# Changelog

All notable changes to **Debuggatha** are documented here.

This project follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)
and [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [Unreleased]

### Added
- **Monorepo Architecture**: Transitioned from a monolithic VS Code extension to a structured Turborepo monorepo.
- **Review Engine**: Fully modularized core engine supporting execution, source/evidence parsing, and detailed review sessions.
- **Repository Intelligence**: Advanced repository scanners for stack detection, dependency analysis, documentation understanding, and deep-freeze snapshotting.
- **Findings Ledger**: Persistent ledger for findings storage, serialization, and lifecycle tracking.
- **Knowledge System**: Context providers and knowledge integration for deeper contextual reviews.
- **Model Context Protocol (MCP)**: Implemented an MCP server with built-in review tools and configurable logging.
- **Extensive Review Packs**: Pre-built review templates for numerous ecosystems including React, Go, Node.js, Electron, performance, and accessibility.
- **AI Skills Framework**: Structured AI skills for specialized tasks such as architecture, diff, and file reviews.
- **Client Implementations**: Developed an extensible CLI tool (`apps/cli`) alongside the newly rebuilt VS Code extension (`apps/vscode`).
- **Policy Engine**: Integrated frameworks for rule, capability, and policy evaluation.

### Removed
- Legacy monolithic codebase structures (`src/`, `webview/`, and `prompts/`) have been fully retired in favor of the modular package ecosystem.

---

## [0.1.0] — 2026-07-10

### Added
- Initial public preview release of the completely rewritten Debuggatha engine.
- Complete domain modeling for static review, capability registries, and persistent ledgers.
- Migration to the unified Sxnnyside Project documentation templates.

---

[Unreleased]: https://github.com/sxnnyside-project/debuggatha/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/sxnnyside-project/debuggatha/releases/tag/v0.1.0
