# 0002. Review Packs are semver-versioned; `ReviewPackReference` carries a real `version` field

## Status

Accepted (revised after product review — see "Amendment" below)

## Context

Review Packs will evolve — CLAUDE.md's own deferred list names a growing
catalog (Flutter, Tauri, Accessibility, Performance, Release packs...).
A `Finding.originatingPack` and `ReviewSession.selectedPacks` already
exist as `ReviewPackReference` — a type Epic 2 shipped.

Two problems need solving:

1. How does a `ReviewPack` declare its own version?
2. How does a reference to "this pack, this exact version" travel through
   `ReviewRequest.requestedPackIds: string[]`, `ReviewPackReference`, and
   `Finding.originatingPack` unambiguously?

## Decision

- `ReviewPack.id` is the bare pack identifier (`"react-ts"`, `"owasp"`) —
  stable across all versions of that pack.
- `ReviewPack.version` is a semver string (`"2.1.0"`), separate from `id`.
- `ReviewPackReference` (in `@debuggatha/review-engine`) is
  `{ id: string; version: string }` — a real, structured field, not a
  string convention. Every place a pack must be referenced with a
  specific version resolved (`ReviewPackReference`, `Finding.originatingPack`)
  uses this shape.
- `ReviewRequest.requestedPackIds: string[]` (already shipped, unchanged)
  stays bare pack ids — a request names *which packs*, not *which
  version*; version resolution is the Capability Registry's job, done at
  Rule Resolution time, before anything is stored on a `ReviewPolicy`,
  `ReviewSession`, or `Finding`. A bare id in a request is shorthand for
  "resolve the latest version the registry has."
- `ReviewPolicyReference` stays `{ id: string }`, unchanged — a policy has
  no independently authored version; it's identified entirely by its own
  deterministic id (ADR-0005).

## Amendment

The original proposal kept `ReviewPackReference` exactly as Epic 2 shipped
it (`{ id: string }`) and encoded the version inside `id` as a
`"packId@version"` string, to avoid touching an "already implemented"
package. Product review rejected that tradeoff: overloading a string field
with a parsing convention was judged worse long-term than a small,
additive, non-breaking field addition to a package that has no external
consumers yet. `ReviewPackReference.version` was added directly instead —
see `packages/review-engine/src/types/reference.ts`. No other Epic 2 type
changed.

## Consequences

- `ReviewPackReference` is self-describing — no `@`-split parsing
  convention exists anywhere in the codebase, and a pack id is free to
  contain any character (no reserved delimiter).
- Every existing construction site of `ReviewPackReference` in
  review-engine's tests needed a `version` field added (small, mechanical,
  already done as part of this decision landing).
- Historical `Finding.originatingPack` values stay meaningful after a pack
  updates: `{ id: "react-ts", version: "2.1.0" }` always means that exact
  version, regardless of what `"react-ts"`'s latest version becomes later
  — see ADR-0005 for how `ReviewPolicy.id` extends this same guarantee to
  a whole resolved policy.
- Adding `version` as a required field means every `ReviewPackReference`
  must resolve to a concrete version before construction — there is no
  "unversioned reference" representable in the type system, which is
  stronger than the string convention's runtime-only guarantee.

## Alternatives considered

- **Encode version inside `id` as `"packId@version"`, leaving
  `ReviewPackReference` untouched.** This was the original proposal;
  rejected on product review (see Amendment) — a real field beats an
  informal string convention when the cost of adding it is this low.
- **Keep pack references always unversioned ("latest wins") and version
  only at the `ReviewPack` definition level.** Rejected: this would make
  `Finding.originatingPack` ambiguous the moment a pack updates — exactly
  the reproducibility problem ADR-0005 and CLAUDE.md's "always citable"
  principle are trying to avoid.
- **A separate lookup table mapping opaque reference ids to
  `{packId, version}` pairs, stored alongside sessions/results.** Rejected
  as unnecessary indirection: it would require its own persistence (which
  doesn't exist yet — see the Findings ledger, still "designed, not yet
  built" per CLAUDE.md) to solve a problem a single struct field already
  solves.
