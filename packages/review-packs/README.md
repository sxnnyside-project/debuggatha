# @debuggatha/review-packs

The real Foundation Bundle content: 29 Review Packs conforming to
`@debuggatha/knowledge-system`'s `ReviewPack` schema. See
[docs/PACK_SPEC.md](../../docs/PACK_SPEC.md) for the canonical spec and
[CLAUDE.md](../../CLAUDE.md) at the repo root for the product decisions
this package encodes.

## Why it exists

Review Skills and the Knowledge System need concrete Rules to resolve
against — this package is where that content lives, separate from the
resolution algorithm (`knowledge-system`) and the wiring that populates
a registry from it (`policies`).

## What it owns

29 pack modules under `src/packs/`, spanning languages (TypeScript,
JavaScript, Rust, Go, Kotlin, Dart, PHP), runtimes (Bun, Node.js),
frontend frameworks (React, Vue, Svelte, Astro, Lit), backend frameworks
(Express, Fastify, Hono, Elysia, Laravel, ASP.NET Core), desktop/mobile
(Tauri, Electron, Flutter), and concern packs (OWASP, Accessibility,
Performance, Architecture, DX, Release) — aggregated into the
`reviewPacks` array.

## What it exposes

`reviewPacks: ReviewPack[]`.

## What depends on it

`@debuggatha/policies` (to populate `defaultCapabilityRegistry`).
Nothing else in the monorepo should import this package directly.

## What should never be implemented here

No resolution logic (Rule Resolution lives in `knowledge-system`), no
registry wiring (lives in `policies`), and no rule-detection logic
(lives in `@debuggatha/skills`'s `RULE_DETECTORS`). This package is
data, not behavior.

## Architectural boundaries

Depends only on `@debuggatha/knowledge-system` (for the `ReviewPack`
type). Never depends on an adapter, on `core`, or on any package that
would create a cycle back to itself.

## Current limitations

Coverage depth varies: 21 packs are substantive with multiple
technology-specific rules citing authoritative sources; 8 (`hono`,
`elysia`, `owasp`, `accessibility`, `performance`, `architecture`,
`dx`) are schema-conformant but thin (3-4 rules each). Test coverage is
1 test for all 29 pack files — schema/loading is verified, individual
pack content is not.

## Future work

Expanding the 8 thin packs is the natural next target, not a defect to
route around.
