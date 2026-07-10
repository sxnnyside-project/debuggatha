# 0005. `ReviewPolicy.id` is a deterministic hash of its resolution inputs

## Status

Accepted

## Context

`ReviewPolicyReference { id: string }` already exists in
`@debuggatha/review-engine` (Epic 2), referenced from
`ReviewSession.selectedPolicy`, `ReviewRequest.requestedPolicyId`, and
`Finding.appliedPolicy`. Epic 2 deliberately left what that `id` actually
*means* to a later epic ("this package never resolves a scope against a
real repository — that's a future epic's job," per its README). This
epic (Knowledge System) is that future epic for policy resolution
specifically.

Two things need to be true simultaneously:

1. Given the same repository state and the same requested Packs, running
   Rule Resolution twice should produce a `ReviewPolicy` a caller can
   recognize as "the same policy" — otherwise `ReviewRequest.requestedPolicyId`
   (re-running a previously assembled policy) has nothing stable to name.
2. A past `Finding.appliedPolicy` must stay meaningful after a Pack it
   was built from later publishes a new version (see ADR-0002) — the
   `id` needs to encode *exactly* which pack versions and which
   repository state produced it, not just "the policy for repo X."

Pulumi's per-resource provider version pinning and Terraform's
plan-then-apply determinism both encode the same underlying property:
identity should be a function of exact inputs, not of "whatever the
current default is right now."

## Decision

`ReviewPolicy.id` is computed as a stable hash (e.g. SHA-256, truncated
for readability) over a canonical serialization of:

- the sorted list of resolved `ReviewPackReference { id, version }` pairs
  actually applied (post-dependency-resolution, post-conflict-resolution —
  the *effective* set, not just what was requested),
- the `RepositoryContext.generatedAt` timestamp plus a content fingerprint
  of `RepositoryContext.criteria` (not the whole `RepositoryContext` —
  only the criteria actually feed Rule Resolution; stack/dependency
  changes that don't affect which criteria rules exist should not
  produce a different policy id),
- a `KNOWLEDGE_SCHEMA_VERSION` constant, bumped whenever the `Rule`/
  `ReviewPack`/`ReviewPolicy` shape itself changes in a way that would
  change resolution output for the same inputs (guards against silently
  reusing an old id under a changed resolution algorithm).

Two `resolvePolicy` calls with identical inputs produce identical `id`s.
This makes `ReviewPolicy` a pure, content-addressed value — resolving is
idempotent and side-effect-free, safe to call repeatedly without a cache
(though nothing prevents caching it later, purely as a performance
optimization, the same way `RepositoryContextCache` optimizes
`buildRepositoryContext` without changing what it returns).

## Consequences

- `ReviewRequest.requestedPolicyId` (re-run a specific past policy) is
  answerable in principle: if the exact same pack versions are still
  registered and the repository's criteria haven't changed, recomputing
  produces the identical id, and Rule Resolution can re-derive the same
  `ReviewPolicy` content on demand rather than needing it stored anywhere
  — no policy store is required for this to work in v1 (see Architecture
  doc §11's YAGNI on a persisted Policy Store).
- If a pack version is later removed from the registry (not just
  superseded — actually deleted), a request naming that exact id can no
  longer be *recomputed*, even though the id itself is still a valid,
  meaningful identifier recorded on old `Finding`s. This is an accepted
  gap: full historical re-resolution durability is the Findings ledger's
  job (CLAUDE.md: "designed, not yet built"), not this epic's.
- Determinism requires resolution to have zero non-deterministic inputs
  — no `Date.now()`-derived randomness inside `resolvePolicy` itself, no
  reliance on iteration order that isn't first made explicit (`sorted...`
  above exists specifically to close this gap; a naive `Object.keys()`
  iteration order bug would silently break reproducibility).

## Alternatives considered

- **A random/UUID `ReviewPolicy.id`, generated fresh every resolution**
  (matching how `ReviewRequest.id` and `Finding.id` already use
  `randomId()` in review-engine). Rejected: would make
  `requestedPolicyId` meaningless (nothing to re-resolve to) and would
  make two runs against an unchanged repository with the same packs
  produce "different" policies with no way to recognize they're
  equivalent — directly undermines "always inspectable, always citable."
- **An incrementing integer/sequence id, assigned by a central
  policy-registration authority.** Rejected: requires a stateful,
  centralized id-issuing service that doesn't exist and isn't otherwise
  needed — content-addressing gets the same "recognize sameness" property
  without any server-side state.
- **Persist every resolved `ReviewPolicy` by id as soon as it's computed**
  (making the id an opaque lookup key into storage rather than a
  self-describing hash). Rejected as premature: this epic has no
  persistence layer yet (that's the deferred Findings ledger), and a
  self-describing content hash gets most of the practical benefit
  (recomputability, stability across identical inputs) without needing
  storage to exist first.
