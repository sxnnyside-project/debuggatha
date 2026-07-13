# @debuggatha/infrastructure

Concrete implementations of ports the domain layer defines but doesn't
implement — currently, git access. See
[CLAUDE.md](../../CLAUDE.md) at the repo root for the product decisions
this package encodes.

## Why it exists

`@debuggatha/core`'s `RepositoryProvider` interface establishes the
boundary between domain logic and version-control access. Something has
to actually shell out to git — this package is where that concrete,
side-effecting code lives, so the rest of the monorepo never has to.

## What it owns

`LocalGitProvider` (`src/providers/git.ts`): a `RepositoryProvider`
implementation backed by the local `git` binary — root dir, diff,
status, and branch.

## What it exposes

`LocalGitProvider`.

## What depends on it

Adapters that need real git access and don't yet route through
`@debuggatha/core`'s `RepositoryProvider` abstraction — currently
`@debuggatha/cli`. `apps/vscode` does **not** depend on this package
yet; it shells out to git directly, a known gap tracked in
[CLAUDE.md](../../CLAUDE.md)'s Epic 6.5 status.

## What should never be implemented here

No domain logic, no review logic, no findings/ledger/policy concerns.
This package is infrastructure only — it should stay swappable (a
future `RemoteGitProvider`, a libgit2 binding) without any domain
package noticing.

## Architectural boundaries

Implements `@debuggatha/core`'s `RepositoryProvider` interface. Does
not depend on any domain package (`review-engine`,
`knowledge-system`, etc.) — only on Node's built-in `child_process`/`fs`
and the interface it implements.

## Current limitations

Only one provider exists (local git via shell-out). No remote/API-based
git provider, no libgit2 binding.

## Future work

Migrating `apps/vscode`'s direct git shell-outs onto
`RepositoryProvider`/`LocalGitProvider` closes the last adapter
bypassing this abstraction.
