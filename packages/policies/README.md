# @debuggatha/policies

Thin wiring between the Knowledge System's policy-resolution algorithm
and the concrete Review Pack catalog. See
[CLAUDE.md](../../CLAUDE.md) at the repo root — "Review Packs" section.

## Why it exists

`@debuggatha/knowledge-system`'s `resolvePolicy` is generic — it takes
any populated `CapabilityRegistry`. Something has to actually populate
one from `@debuggatha/review-packs`' static catalog and hand callers a
single, ready-to-use entry point. That's the entirety of this package.

## What it owns

- `defaultCapabilityRegistry`: the one populated `CapabilityRegistry`
  for v1 (ADR-0004), built once at module load from
  `@debuggatha/review-packs`.
- `assemblePolicy(context, request, registry?)`: calls
  `resolvePolicy` from `@debuggatha/knowledge-system` against that
  registry (or a caller-supplied one).

## What it exposes

`assemblePolicy` and `defaultCapabilityRegistry`, both re-exported
through `@debuggatha/core`.

## What depends on it

`@debuggatha/core`, which re-exports it for `@debuggatha/mcp`,
`@debuggatha/cli`, and `apps/vscode`.

## What should never be implemented here

No policy-resolution algorithm — that's `resolvePolicy`'s job in
`@debuggatha/knowledge-system`. No Review Pack content — that's
`@debuggatha/review-packs`' job. This package should stay small enough
to read in one sitting.

## Architectural boundaries

Depends on `@debuggatha/knowledge-system` (for `resolvePolicy` and its
types) and `@debuggatha/review-packs` (for pack content). Never depends
on an adapter package.

## Current limitations

No `SkillDescriptor` catalog is registered yet — `@debuggatha/skills`
doesn't export one — so a Review Pack declaring a `{ skillId }`
dependency fails closed until that catalog exists.

## Future work

Registering a real `SkillDescriptor` catalog once `@debuggatha/skills`
exposes one.
