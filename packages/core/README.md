# @debuggatha/core

The stable façade every adapter (`@debuggatha/mcp`, `@debuggatha/cli`,
`apps/vscode`) depends on instead of wiring five-plus domain packages
individually. See [CLAUDE.md](../../CLAUDE.md) at the repo root for the
product decisions this package encodes.

## Why it exists

Without this package, every adapter would need to know about
`repository-intelligence`, `review-engine`, `knowledge-system`,
`findings-ledger`, `repository-memory`, `runtime-engine`, `policies`, and
`skills` individually, and would each re-hand-roll the same orchestration
sequence. This package exists to be that one seam.

## What it owns

- **Re-exports**: the public surface of every domain package, so an
  adapter imports one module instead of eight.
- **`executeReview`** (`pipeline.ts`): the one real orchestration
  function — build a `RepositoryContext`, assemble a `ReviewPolicy`,
  create a `ReviewRequest`/`ReviewSession`, run a Review Skill, load
  `.debuggatha/memory.json` and filter suppressed findings, transition
  the session, and sync the result into `.debuggatha/ledger.json`. All
  three adapters call through this instead of each reimplementing the
  sequence.
- **`RepositoryProvider`** (`provider.ts`): the interface adapters
  implement to supply repository access (root dir, diff, status,
  branch) without this package or its dependents shelling out to git
  directly.

## What depends on it

`@debuggatha/mcp`, `@debuggatha/cli`, and `apps/vscode` all depend on
this package as their sole entry point into Debuggatha's domain logic.

## What should never be implemented here

- No transport-specific code (no MCP tool schemas, no CLI argument
  parsing, no VS Code API calls). Those belong in the adapters.
- No new domain logic. If a rule, a lifecycle transition, or a
  capability check doesn't already exist in the package it re-exports
  from, it doesn't belong here either — add it upstream, then re-export.

## Architectural boundaries

Depends on every domain package (`repository-intelligence`,
`review-engine`, `knowledge-system`, `findings-ledger`,
`repository-memory`, `runtime-engine`, `policies`, `skills`,
`analysis-engine`, `context-intelligence`); nothing outside this package
tier depends on it except the three adapters.

## Current limitations

`executeReview` is imperative orchestration, not itself covered by a
domain-level lifecycle or event system — see `pipeline.test.ts` for what
is verified today.

## Future work

As new epics land, re-export their public surface here rather than
having adapters reach into the package directly.
