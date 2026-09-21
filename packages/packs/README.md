# @debuggatha/packs

Review Pack content: the rules Review Skills resolve against, conforming to
`core/knowledge-system`'s `ReviewPack` schema. See [docs/PACK_SPEC.md](../../docs/PACK_SPEC.md) for
the spec.

## What it owns

Pack modules under `src/packs/`, aggregated into the `reviewPacks` array:

- Languages: TypeScript, JavaScript, Rust, Go, Kotlin, Dart, PHP
- Runtimes: Bun, Node.js
- Frontend: React, Vue, Svelte, Astro, Lit
- Backend: Express, Fastify, Hono, Elysia, Laravel, ASP.NET Core
- Desktop and mobile: Tauri, Electron, Flutter
- Concerns: OWASP, Accessibility, Performance, Architecture, DX, Release

Every rule is atomic, individually citable, and points to an authoritative source. Depth varies by
pack: some carry many technology-specific rules, others (`hono`, `elysia`, `owasp`,
`accessibility`, `performance`, `architecture`, `dx`) a few.

## Boundaries

Data, not behavior. No resolution logic (`core/knowledge-system`), no registry wiring
(`engine/policies`), and no rule detection (`engine/skills`). Depends only on `@debuggatha/core`
for the `ReviewPack` type.
