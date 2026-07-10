# 0001. A new `@debuggatha/knowledge-system` package owns Pack/Policy domain logic

## Status

Accepted

## Context

Epic 1 (`@debuggatha/repository-intelligence`) and Epic 2
(`@debuggatha/review-engine`) each established a pattern: a pure,
provider-independent domain package with no dependency on MCP, the CLI,
VS Code, or any LLM. `packages/review-packs` and `packages/policies`
already exist as scaffolds (see their current placeholder
`src/index.ts`/`src/types.ts`), but both currently depend on
`@debuggatha/shared`'s legacy `PolicyRule`/`ReviewPolicy` types — the
same superseded vocabulary `review-engine`'s README already flags as
migration debt.

Epic 3 needs to define: `SkillDescriptor`, `ReviewPack`, `Rule`,
`KnowledgeEntry`, a `CapabilityRegistry`, Rule Resolution
(`resolvePolicy`), Context Assembly (`assembleContext`), `ReviewPolicy`,
and conflict handling. These are domain concepts, not content and not
orchestration wiring — mixing them into `review-packs` (content) or
`policies` (a thin call site) would repeat the exact problem
`@debuggatha/shared` already has: logic and content sharing one package
makes both harder to version and test independently.

## Decision

Create `@debuggatha/knowledge-system` (alias `@knowledge-system`,
following the existing `@repo-intel`/`@review-engine` alias convention in
`tsconfig.base.json`) as the pure-domain package for Epic 3. It:

- Depends on `@debuggatha/repository-intelligence` (for
  `RepositoryContext`, `CriteriaRule`) and `@debuggatha/review-engine`
  (for `Evidence`, `Category`, `Severity`, `ReviewPackReference`,
  `ReviewPolicyReference`) — the same direction Epic 2 already depends on
  Epic 1.
- Is depended on by `@debuggatha/review-packs` (which conforms its
  content to `knowledge-system`'s `ReviewPack` schema),
  `@debuggatha/policies` (which wires a populated `CapabilityRegistry` to
  `resolvePolicy`), and `@debuggatha/skills` (which consumes
  `SkillContext` and cites `Rule`/`KnowledgeEntry` ids in `Finding`
  evidence).
- Has no dependency on MCP, the CLI, VS Code, or any LLM provider — same
  discipline as Epics 1 and 2.

`@debuggatha/review-packs` becomes content-only (a library of `ReviewPack`
values); `@debuggatha/policies` becomes a thin orchestration call site
(wires a registry instance to `resolvePolicy`, nothing more).

## Consequences

- Mirrors an already-proven pattern in this codebase (Epic 1 / Epic 2
  package boundaries), so the team already knows the shape: domain
  package with pure functions and frozen types, thin consumer packages
  around it.
- `knowledge-system` can be fully unit-tested (Rule Resolution, conflict
  detection, dependency resolution) without importing any real pack
  content — mirrors how `review-engine`'s tests don't need real
  `RepositoryContext` scans, just constructed fixtures.
- Adds a fifth "Epic package" to the dependency graph
  (`repository-intelligence` → `review-engine` → `knowledge-system`),
  which `packages/core`'s façade will need to re-export alongside the
  existing two, following the same pattern already established in
  `core/src/index.ts`.
- `packages/skills` and `packages/policies` both need to drop their
  `@debuggatha/shared` imports and move onto `review-engine` +
  `knowledge-system` types as part of this epic — this was already
  called out as owed migration debt in `review-engine`'s README "Risks";
  this ADR makes it concrete rather than open-ended.

## Alternatives considered

- **Put Pack/Policy domain logic directly in `@debuggatha/policies`.**
  Rejected: `policies` would then need to be the thing every consumer
  (skills, a future MCP layer) imports just to get type definitions,
  even before any actual policy assembly is needed, and testing the
  algorithm would require importing whatever pack content
  `review-packs` happens to ship. Keeping the algorithm's package
  separate from both the content package and the "wire it up" package
  keeps each testable and versionable independently.
- **Fold Pack/Policy types into `@debuggatha/review-engine` directly
  (extend `ReviewPackReference`/`ReviewPolicyReference` in place).**
  Rejected: Epic 2 is implemented and explicitly out of scope to modify
  for this task; review-engine's whole design point was staying
  independent from Review Packs/Policies (its README: "This package does
  not... implement Review Packs or Review Policies — it only references
  them by id"). Reopening that boundary would undo a decision Epic 2
  already made deliberately.
- **No new package — extend `@debuggatha/shared`.** Rejected outright:
  `shared` is the legacy package review-engine's README already
  documents as superseded and slated for removal once Review
  Packs/Policies are implemented. Adding new domain logic to a package
  already marked for deprecation would create more migration debt, not
  less.
