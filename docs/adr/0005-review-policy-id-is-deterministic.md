# 0005. `ReviewPolicy.id` is a deterministic hash of its resolution inputs

## Status

Accepted

## Context

`ReviewPolicyReference { id }` is referenced from `ReviewSession.selectedPolicy`, `ReviewRequest.requestedPolicyId`, and `Finding.appliedPolicy`. Two things must hold:

1. Resolving twice from the same repository state and the same requested packs yields a policy a caller recognizes as the same one; otherwise `requestedPolicyId` has nothing stable to name.
2. A past `Finding.appliedPolicy` stays meaningful after a pack publishes a new version (ADR-0002): the id must encode exactly which pack versions and which repository criteria produced it.

Pulumi's provider version pinning and Terraform's plan-then-apply determinism encode the same property: identity is a function of exact inputs, not of whatever the default is now.

## Decision

`ReviewPolicy.id` is a stable hash (SHA-256, truncated) over a canonical serialization of:

- the sorted list of resolved `ReviewPackReference { id, version }` pairs actually applied (after dependency and conflict resolution: the effective set, not the requested one),
- the `RepositoryContext.generatedAt` timestamp plus a content fingerprint of `RepositoryContext.criteria` (only the criteria feed Rule Resolution, so stack or dependency changes that do not affect them do not change the id),
- a `KNOWLEDGE_SCHEMA_VERSION` constant, bumped when the `Rule`, `ReviewPack`, or `ReviewPolicy` shape changes in a way that would change resolution output for the same inputs.

Identical inputs produce identical ids. `ReviewPolicy` is a pure, content-addressed value: resolving is idempotent and side-effect free.

## Consequences

- A policy can be re-derived on demand from the registry and the repository's criteria; no policy store is needed.
- If a pack version is removed from the registry, a request naming that id can no longer be recomputed, although the id remains a valid identifier on old findings. The ledger's history is what keeps old findings meaningful.
- Resolution has no non-deterministic inputs: no time-derived randomness inside `resolvePolicy`, and no reliance on unspecified iteration order (inputs are sorted).

## Alternatives considered

- **A random id per resolution.** Rejected: `requestedPolicyId` would be meaningless, and equivalent runs would look different.
- **An incrementing id from a central authority.** Rejected: it needs a stateful service that does not otherwise exist; content-addressing gives the same recognizability without state.
- **Persist every resolved policy and use the id as an opaque key.** Rejected as premature: a self-describing hash provides recomputability and stability without storage.
