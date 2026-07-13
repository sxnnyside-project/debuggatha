# @debuggatha/cli

The standalone `debuggatha` command — terminal and CI usage, no editor
required. See [CLAUDE.md](../../CLAUDE.md) at the repo root for the
product decisions this package encodes.

## Why it exists

MCP and VS Code both assume an interactive host. CI pipelines and
terminal-first workflows need a way to run the same reviews and inspect
the same ledger without either.

## What it owns

Command registration and terminal-facing concerns only: argument
parsing (`commander`), output formatting (human-readable and `--json`),
and exit codes (`0` clean, `2` active findings found, `1` unexpected
error).

Commands: `review` (diff/files/workspace), `findings`
(list/show/resolve/dismiss/reopen/summary), `memory`
(list/suggest/confirm/activate/reject/deprecate/archive/remove),
`repository`, `packs`, `policies`, `doctor`, `init`.

## What it exposes

The `debuggatha` binary (`bin` entry in `package.json`), installable via
`bunx @debuggatha/cli` or as a workspace dependency.

## What depends on it

Nothing inside the monorepo — this is a leaf, consumed only by end
users and CI.

## What should never be implemented here

No business logic, domain constraints, or review generation. Every
command strictly orchestrates `@debuggatha/core`; if a command needs
logic that isn't already in `core` or the package it re-exports from,
that logic belongs upstream, not in a command handler.

## Architectural boundaries

Depends only on `@debuggatha/core`. Never imports a domain package
(`review-engine`, `findings-ledger`, etc.) directly — always through
`core`'s façade.

## Current limitations

`doctor` covers Node version, git presence, `.debuggatha` presence, and
context-build success — evidence-trail and policy-conformance checks
are not yet implemented.

## Future work

Additional `doctor` checks; richer `--json` output for CI integrations
that parse findings programmatically.
