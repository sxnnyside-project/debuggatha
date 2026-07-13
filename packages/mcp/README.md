# @debuggatha/mcp

The MCP server — Debuggatha's default distribution surface for MCP
hosts (Claude Code, Claude Desktop, Cursor, Copilot agent mode). See
[CLAUDE.md](../../CLAUDE.md) at the repo root: "MCP is distribution
only, not the value proposition."

## Why it exists

Hosts that speak MCP need a stdio server exposing tools, resources, and
prompts. This package is that adapter — nothing more.

## What it owns

Tool handlers (`review_diff`, `review_files`, `review_workspace`,
`list_findings`, `get_finding`, `update_finding`, `repository_context`,
`repository_summary`), resource exposure (repository context/summary,
ledger, ledger history), prompts (`review-flow`, `triage-findings`),
config loading (`config.ts`), structured logging (`logging.ts`), and
error sanitization (`errors.ts` — raw fs paths and stack traces never
reach the client).

## What it exposes

The `debuggatha-mcp` binary (`bin` entry in `package.json`), and
`createServer`/`createDefaultDomainDeps`/`loadConfig`/`createLogger` for
embedding.

## What depends on it

Any MCP host configured to launch it. Nothing else in the monorepo
depends on this package.

## What should never be implemented here

Zero business logic — every tool handler delegates to
`@debuggatha/core` via an injected `DomainDeps`. This package should
never gain a reverse import from any domain package; if a handler needs
new logic, it belongs in `core` or upstream of it.

## Architectural boundaries

Depends only on `@debuggatha/core`. Owns the MCP-specific concerns
(tool schemas, resource URIs, prompt templates, transport) that no
other package should need to know about.

## Current limitations

Real MCP sampling — using the host's already-configured model as the
default review runtime — is not wired end-to-end yet; this is the
top-flagged item in [CLAUDE.md](../../CLAUDE.md)'s "Implementation
status" (Epic 16A/16B).

## Future work

Wiring MCP sampling; expanding resource coverage as new domain packages
land.
