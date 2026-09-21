# 0004. The Capability Registry is an injectable interface with an in-memory default

## Status

Accepted

## Context

Skills and Review Packs need a place that answers "what exists, and what does it need" before anything is resolved or executed, the way LSP's `initialize`, MCP's `tools/list`, and `terraform providers schema` describe capability without executing anything.

`RepositoryContextCache` already sets the precedent: injectable, not a global singleton; callers construct their own instance and control its lifetime. A global mutable registry would make tests leak into each other and presume one deployment topology.

There is no third-party pack author, plugin marketplace, or filesystem or network discovery requirement.

## Decision

`CapabilityRegistry` is an interface in `core/knowledge-system`:

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

`createInMemoryRegistry(packs, skills)` is the default implementation. `engine/policies` builds one instance from `@debuggatha/packs` and the skill descriptors and calls `resolvePolicy` against it. There is no filesystem scanning, network fetch, or dynamic registration.

## Consequences

- Resolution, conflict handling, and dependency resolution are testable from small in-memory fixtures.
- A filesystem-discovered or network-backed registry, if ever needed, is a new implementation of the interface, not a breaking change to `resolvePolicy`.
- There is one populated registry per process, assembled from static imports: the least dynamic option that still respects the domain boundary.

## Alternatives considered

- **A global singleton registry.** Rejected: hidden global state makes test isolation fragile.
- **Filesystem-based pack discovery.** Rejected as premature: no external pack author exists to justify path resolution, malformed-file handling, and reload semantics.
- **An in-process client/server capability negotiation protocol.** Rejected: there is no process boundary between the registry and its consumers, so a wire protocol adds cost with no benefit.
