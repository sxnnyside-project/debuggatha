# 0004. Capability Registry is an injectable interface with a static in-memory default

## Status

Accepted

## Context

Skills (Core/Review/Analysis) and Review Packs both need a place that
answers "what exists, and what does it need" before anything is
resolved or executed — the same role `ClientCapabilities`/
`ServerCapabilities` play in LSP's `initialize` handshake, `tools/list`
plays in MCP, and `terraform providers schema` plays for Terraform:
describe capability without executing anything.

`@debuggatha/repository-intelligence` already established a directly
relevant precedent in this exact codebase: `RepositoryContextCache` is
"injectable, not a global singleton... callers construct their own cache
instance and control its lifetime" (that package's README, "Technical
decisions"). The same tension exists here — a global mutable registry
would make tests hard to isolate (one test registering a pack would leak
into another) and would presume a single deployment topology (in-process
monorepo) that Debuggatha may not keep forever.

v1's scope is small: 2–3 Review Packs, four Core Skills (already
shipped), Diff Review + Architecture Review as the only two Review
Skills. There is no third-party pack author yet, no plugin marketplace,
no filesystem or network discovery requirement.

## Decision

`CapabilityRegistry` is defined as an interface in
`@debuggatha/knowledge-system`:

```ts
interface CapabilityRegistry {
  listSkills(tier?: SkillTier): SkillDescriptor[];
  listPacks(kind?: ReviewPackKind): ReviewPack[];
  getPack(id: string, versionRange?: string): ReviewPack | undefined;
  resolveDependencies(packIds: string[]): {
    resolved: ReviewPack[];
    missing: PackDependency[];
  };
}
```

A default `createInMemoryRegistry(packs: ReviewPack[], skills:
SkillDescriptor[])` implementation ships alongside it — same naming
pattern as repository-intelligence's `createInMemoryCache`. For v1,
`@debuggatha/policies` constructs one instance at module load, populated
from `@debuggatha/review-packs`' exported catalog and
`@debuggatha/skills`' exported descriptors, and that instance is what
`resolvePolicy` is called against. No filesystem scanning, no network
fetch, no dynamic registration API in v1.

## Consequences

- Tests for Rule Resolution, conflict handling, and dependency resolution
  can construct a `CapabilityRegistry` from small in-memory fixtures
  without importing any real pack content — same benefit
  `RepositoryContextCache`'s injectability already gives Epic 1's test
  suite.
- The interface boundary means a future filesystem-discovered or
  network-backed registry (if Debuggatha ever needs third-party packs) is
  an additive change — a new `CapabilityRegistry` implementation — not a
  breaking change to `resolvePolicy` or anything that consumes the
  registry.
- v1 has exactly one populated registry instance per process, assembled
  once from static imports. This is intentionally the least dynamic
  option that still satisfies the domain boundary — matches CLAUDE.md's
  "do not build the full Review Pack catalog before v1 validates the core
  hypothesis."

## Alternatives considered

- **A global singleton registry (module-level mutable state), mirroring
  how many linter/plugin systems register via side-effecting imports.**
  Rejected for the same reason Epic 1 rejected it for
  `RepositoryContextCache`: hidden global state makes test isolation
  fragile and presumes a single process/topology indefinitely.
- **Filesystem-based pack discovery from day one** (scan a `packs/`
  directory or `node_modules` for anything conforming to the schema, the
  way Biome discovers `.grit` files from `biome.json`'s `plugins` array).
  Rejected as premature: no external pack author exists yet to make this
  worth the added complexity (path resolution, malformed-file handling,
  hot-reload semantics) — flagged explicitly as YAGNI in the Architecture
  doc §11.
- **Full MCP-style client/server capability negotiation protocol
  in-process** (JSON-RPC-shaped `initialize`/`listChanged` messages
  between the registry and its consumers). Rejected: there is no process
  boundary between the registry and its consumers in v1 (all in the same
  Node/Bun process) — a wire protocol would add serialization overhead
  and complexity with no corresponding benefit until packs genuinely need
  to live out-of-process.
