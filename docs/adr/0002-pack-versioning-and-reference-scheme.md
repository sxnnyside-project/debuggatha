# 0002. Review Packs are semver-versioned; `ReviewPackReference` carries a real `version` field

## Status

Accepted

## Context

Review Packs evolve. A `Finding.originatingPack` and `ReviewSession.selectedPacks` are `ReviewPackReference`s, so two questions need answers: how does a pack declare its version, and how does a reference to "this pack, this exact version" travel unambiguously through requests, sessions, and findings?

## Decision

- `ReviewPack.id` is the bare pack identifier (`"debuggatha/react"`), stable across versions.
- `ReviewPack.version` is a semver string, separate from `id`.
- `ReviewPackReference` (in `core/review-engine`) is `{ id: string; version: string }`, a structured field rather than a string convention. Everywhere a pack is referenced at a resolved version (`ReviewPackReference`, `Finding.originatingPack`) uses this shape.
- `ReviewRequest.requestedPackIds` stays bare ids: a request names which packs, not which version. Version resolution is the Capability Registry's job, done at Rule Resolution time, before anything is stored on a policy, session, or finding.
- `ReviewPolicyReference` stays `{ id: string }`: a policy has no independently authored version and is identified by its deterministic id (ADR-0005).

## Consequences

- No `@`-split parsing convention exists anywhere, and a pack id may contain any character.
- `Finding.originatingPack` stays meaningful after a pack updates: `{ id, version: "2.1.0" }` always means that exact version.
- There is no unversioned reference in the type system; every reference resolves to a concrete version before construction.

## Alternatives considered

- **Encode the version inside `id` as `"packId@version"`.** Rejected: overloading a string with a parsing convention is worse than one additive field.
- **Unversioned references ("latest wins").** Rejected: `Finding.originatingPack` would become ambiguous the moment a pack updates, breaking reproducibility and "always citable."
- **A lookup table from opaque reference ids to `{ packId, version }`.** Rejected: indirection with its own persistence, to solve what a single struct field already solves.
