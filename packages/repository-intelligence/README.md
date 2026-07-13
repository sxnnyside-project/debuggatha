# @debuggatha/repository-intelligence

Epic 1. Produces a `RepositoryContext` — a normalized, immutable snapshot
of a repository's stack, dependencies, documentation, and engineering
criteria — before any review happens. See [CLAUDE.md](../../CLAUDE.md) at
the repo root for the product decisions this package encodes.

This package does **not** review code. It does not know what a "Finding"
or a "Review Policy" is. It has no dependency on MCP, VS Code, the CLI, or
any LLM provider — every fact it produces is deterministic and evidence-backed.

## Architecture summary

One entrypoint, `buildRepositoryContext(rootDir, options?)`, runs four
independent scanners once and combines their output:

```text
buildRepositoryContext(rootDir)
├── scanStack          → StackProfile        (languages, frameworks, build systems,
│                                              package managers, runtimes, workspace
│                                              type, platform targets)
├── scanDependencies    → DependencyProfile   (lockfile presence, declared versions
│                                              for recognized frameworks/runtimes)
├── scanDocumentation  → DocumentationProfile (README/CLAUDE.md/ADRs/PRD/roadmap/docs)
├── scanCriteria         → CriteriaProfile      (lint/format config + rule-shaped docs)
└── deriveUnderstanding → UnderstandingProfile (open questions, confidence — pure
                                                 function of the four profiles above)
```

`scanStack` reads `package.json`/`Cargo.toml`/`pubspec.yaml` once and hands
the parsed result to `scanDependencies`, so nothing is read or parsed
twice in a single call. The five profiles are assembled into one
`RepositoryContext`, deep-frozen (`Object.freeze`, recursively — not just
TypeScript's compile-time `readonly`), and optionally cached.

Every scanner returns `{ profile, filesRead, dirsListed }` — not just its
profile — so the orchestrator can build a **fingerprint** of exactly what
it read: every directory it listed and every file it opened, each with an
`mtimeMs`/`size` stamp. `RepositoryContextCache` is injectable (no hidden
global singleton); on the next call for the same root, the fingerprint is
cheaply re-checked (stat calls, not re-reading file contents) and the
cached `RepositoryContext` is returned unchanged — verified in tests by
referential equality (`result2 === result1`), not by inspecting internals.

## Repository Capability Resolution (Epic 12.5)

`RepositoryContext.capabilities` (`Capability[]`, via
`deriveCapabilities` in `capabilities.ts`) is the canonical,
machine-matchable contract Review Pack resolution and Review Policy
assembly consume — `StackProfile` above remains a human-readable,
display-oriented representation kept for backwards compatibility, but it
is no longer what capability matching resolves against.

```text
scanStack → StackScan (StackProfile + parsed package.json/Cargo.toml/pubspec.yaml)
        │
        ▼
deriveCapabilities(rootDir, rootEntries, stackScan)
        │  normalizes each StackSignal into a lowercase, alphanumeric-only id
        │  ("React" -> "react", "Next.js" -> "nextjs", "ASP.NET Core" -> "aspnetcore")
        │  splits StackProfile's single "JavaScript/TypeScript" signal into two
        │  independent capabilities (see "The Epic 11 fix" below)
        │  adds capabilities StackProfile never tracked: PHP/Laravel (composer.json),
        │  a VS Code Extension platform (package.json engines.vscode), tooling
        │  (biome/eslint/vitest config files), and repository characteristics
        │  (monorepo/package-based/feature-based/layered)
        ▼
Capability[] { id, kind, confidence, evidence, origin }
        │
        ▼
resolvePolicy (@debuggatha/knowledge-system) matches each Rule's
`requires-language`/`requires-framework` scope against this flat id set
        │
        ▼
summarizeCapabilities(capabilities) → human-readable summary string
(grouped by kind, sorted — the display label is now an *output* of the
capability model, never its input)
```

**Additive, never mutually exclusive** — a repository with TypeScript,
JavaScript, React, Bun, Turborepo, and Tauri gets six independent
`Capability` entries, not one chosen "stack" label. This is a load-bearing
design constraint, not an implementation detail: it's what lets a
polyglot or hybrid repository (multiple languages, multiple frameworks,
a monorepo containing several stacks at once) resolve every applicable
Review Pack simultaneously instead of collapsing to a single guess.

**The Epic 11 fix.** Epic 11's Architecture Review surfaced that the
`debuggatha/typescript`/`debuggatha/javascript` stack packs' rules could
never resolve for any real repository: `StackProfile.languages` carries
one combined `"JavaScript/TypeScript"` signal (a `package.json`'s mere
presence), but the packs declare separate `requires-language: "typescript"`
/`"javascript"` scopes, and the old exact (case-insensitive) string match
against `stack.languages` never equaled either. `deriveCapabilities`
splits this signal: every `package.json`-containing repo gets a
`"javascript"` capability unconditionally, and a `"typescript"`
capability *additionally* when `tsconfig.json` is present (confidence
`"high"`) or `"typescript"` is a declared dependency (confidence
`"medium"`) — never both, and never neither when there's no evidence.
`packages/core/src/pipeline.test.ts`'s "Epic 12.5 capability fix" test
proves this end-to-end: `no-explicit-any` (a `debuggatha/typescript`
rule) now actually produces a `Finding` against a real TypeScript file.

**Confidence and "unknown is preferable to incorrect."** Every
`Capability` carries its own `confidence` independent of the others —
`detectTypeScript`'s medium-vs-high split above is the clearest example.
A capability with no supporting evidence is never emitted at all (there
is no `"unknown"` capability value — absence *is* the "unknown" signal),
matching Stack Detection's own evidence discipline from Epic 1.

## Implemented capabilities

- **Stack Detection** — languages, frameworks, build systems, package
  managers, runtimes, workspace type (single-package/monorepo), and
  platform targets (web/desktop/mobile). Every signal carries an
  `Evidence[]` trail; ambiguous filenames are resolved by content, not
  presence alone (a `Cargo.toml` is only tagged "Tauri" if it actually
  depends on `tauri`; `pubspec.yaml` is only tagged "Flutter" if it
  depends on `flutter` — plain Dart otherwise).
- **Dependency Context** — lockfile presence (no content parsing), and
  declared version constraints, but only for the frameworks/runtimes
  Stack Detection already recognized (not the entire dependency tree —
  this is metadata, not a dependency audit).
- **Documentation Context** — discovers README, CLAUDE.md, CONTRIBUTING.md,
  ARCHITECTURE.md, ROADMAP.md, and recursively walks `docs/` (bounded
  depth, skipping `node_modules`/`.git`/`dist`/etc.) classifying ADRs,
  PRDs, and roadmaps by path/filename convention. Produces a factual,
  mechanically-generated summary sentence — never an opinion.
- **Criteria Resolution** — JSON-shaped configs (`.eslintrc(.json)`,
  `biome.json`, `.prettierrc(.json)`) are parsed into real structured
  rules. `.editorconfig` gets a real INI-style parser. `rustfmt.toml`
  gets a best-effort flat `key = value` extractor. Rule-shaped Markdown
  (`ARCHITECTURE.md`, `CONTRIBUTING.md`, `CLAUDE.md`, `*_CRITERIA.md`) is
  structured at heading granularity — see "Risks" below for what that
  does and doesn't mean.
- **Repository Understanding** — a pure function over the four profiles
  above. Never guesses: it only asks a question when a concrete gap
  exists (no language detected, no runtime marker for a JS/TS project,
  no documentation, no criteria), and confidence degrades as questions
  accumulate.
- **Repository Snapshot + Cache** — the immutable `RepositoryContext` and
  the fingerprint-based, injectable cache described above.

## Deferred (explicitly out of scope for this epic)

- Review Engine: Findings, Review Packs, Policies, Architecture/Diff
  Review — `@debuggatha/skills`' `review/` stubs are untouched.
- MCP, CLI, VS Code wiring — `@debuggatha/core` re-exports this package's
  public API today; it adds no orchestration logic of its own yet.
- Lockfile *content* parsing (resolved/transitive versions) — see Risks.
- AI-assisted answers to `UnderstandingProfile.openQuestions` — this
  package only produces the questions; nothing here calls a model.

## Technical decisions

- **No YAML/TOML parser dependency.** `.editorconfig` (a well-defined,
  simple INI-like format) gets a real hand-written parser. `rustfmt.toml`
  gets a bounded flat `key = value` extractor — correct for typical
  single-table config files, not a TOML spec implementation. Pulling in a
  full parser for one or two config shapes was judged not worth the
  dependency weight; see Risks for where this bites.
- **`RepositoryContextCache` is injectable, not a global singleton.**
  Callers (a future MCP server, a CLI run, a test) construct their own
  cache instance and control its lifetime. `buildRepositoryContext`
  without a `cache` option always rescans — correctness over convenience
  by default.
- **The fingerprint tracks directory listings, not just file stamps.** A
  naive cache that only stat'd previously-read files would miss a brand
  new `Cargo.toml` appearing in a repo that previously had none (there's
  no previous stamp to compare). Tracking the sorted directory listing
  the scan actually consulted closes that gap; see `snapshot.test.ts` →
  "invalidates the cache when a new manifest appears."
- **Rule-shaped Markdown is structured at heading granularity only.**
  Extracting "## No unwrap() in production code" from CONTRIBUTING.md as
  a `CriteriaRule` is honest: it says a rule-shaped section exists and
  names it. It does not claim to understand the prose underneath. Doing
  that would require actual language understanding, which this package
  deliberately excludes (see CLAUDE.md — "No LLM reasoning should be
  required for deterministic information").
- **Dependency Context tracks a fixed allowlist of packages**
  (`react`, `next`, `vue`, `svelte`, `electron`, the two `@tauri-apps/*`
  packages), not every declared dependency. Surfacing the whole tree
  would start to look like the security/audit surface the epic
  explicitly excluded.

## Risks

- **`.eslintrc` without an extension can be YAML or a JS module, not just
  JSON.** This package only attempts `JSON.parse` on it; a YAML-flavored
  `.eslintrc` is silently skipped (no rules extracted, no crash). Same
  risk applies to any `.prettierrc` written in YAML.
- **`detekt.yml` is recorded as present but never parsed.** Kotlin/JVM
  projects using detekt get a single `detekt:present` rule with no
  detail. Closing this gap means either a YAML parser dependency or a
  hand-rolled reader bounded to detekt's specific shape — deferred.
- **`rustfmt.toml` parsing breaks on nested tables or arrays** (anything
  beyond flat `key = value` lines). Typical `rustfmt.toml` files are
  flat, so this covers the common case, but a table-shaped config would
  silently lose entries rather than error.
- **Heading-level extraction from Markdown can't tell a real rule from an
  unrelated section.** A CONTRIBUTING.md with a "## Acknowledgements"
  heading becomes a `CriteriaRule` with that description. This is a
  structural, not semantic, extraction — consumers of `CriteriaProfile`
  need to treat `description` as "the name of a section that might be a
  rule," not as a verified rule.
- **Platform target inference is additive-only by design** (a framework
  positively implies "web"/"desktop"/"mobile"; absence of one implies
  nothing). A backend-only Go or Rust service correctly gets an empty
  `platformTargets` rather than a guessed `"backend"` — this is
  intentional (see CLAUDE.md "never opine without evidence"), but it
  means `platformTargets` cannot currently be used to positively identify
  backend/library projects.
- **The in-memory cache does not persist across process restarts.** Every
  new MCP server / CLI invocation starts cold. This was explicitly out of
  scope ("Correctness is more important than performance" — a disk-backed
  cache introduces its own correctness questions, like stale cache
  surviving a `git checkout`, that are better solved deliberately later).
- **(Epic 12.5) Capability derivation still covers a subset of the
  Foundation Bundle's frameworks.** ASP.NET Core (`aspnetcore`) has no
  detector at all — .NET project files (`.csproj`) aren't scanned by
  Stack Detection or `deriveCapabilities`, so any pack rule scoped
  `requires-framework: "aspnetcore"` still never resolves. PHP/Laravel
  detection reads `composer.json` directly in `capabilities.ts` rather
  than through `scanStack`/`StackScan` (Stack Detection itself has no
  PHP/Composer awareness) — functionally correct, but a source of
  Composer parsing living outside Epic 1's own scanner, worth folding in
  if PHP/Laravel support needs to grow further.
- **Characteristic detection (monorepo/package-based/feature-based/
  layered) is directory-name-based, not structural** — the same
  limitation `@debuggatha/analysis-engine`'s Module Boundaries (Epic 12)
  fully documents for its own, more thorough version of this same
  heuristic. This package's version is intentionally shallow (one level
  of `src/*` inspection) since Repository Intelligence's job is fast
  capability resolution, not deep structural analysis — that's Epic 12's
  job.
