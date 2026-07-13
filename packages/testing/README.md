# @debuggatha/testing

Shared test fixtures for packages that need real files on disk, not
mocked `fs` calls. See [CLAUDE.md](../../CLAUDE.md) at the repo root for
the product decisions this package encodes.

## Why it exists

Core Skills like Stack Detection and Criteria Resolution read real
manifests and config files. Mocking `fs` for every test would drift
from actual filesystem behavior; this package gives every package a
single, consistent way to materialize a throwaway repository for a
test.

## What it owns

`withTempRepo(files)`: writes a `Record<path, contents>` into a fresh
temp directory (via `mkdtempSync`) and returns `{ root, cleanup }`.

## What it exposes

`withTempRepo`.

## What depends on it

Any package's test suite that needs a real repository on disk —
`repository-intelligence`, `context-intelligence`, `analysis-engine`,
`skills`, `core`, among others.

## What should never be implemented here

No assertions, no test runner configuration, no domain-specific
fixtures (a "fixture repo with a React stack" belongs in the test file
that needs it, built with `withTempRepo`, not hardcoded here). This
package stays generic.

## Architectural boundaries

Depends only on Node built-ins (`node:fs`, `node:os`, `node:path`). Not
a runtime dependency of any shipped package — `devDependency` only.

## Current limitations

Single fixture-building function; no fixture teardown registry beyond
the returned `cleanup()` (callers are responsible for calling it, e.g.
in a test's `afterEach`).

## Future work

None planned — this package is intentionally minimal and should stay
that way unless a second genuinely shared testing need appears.
