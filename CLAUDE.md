# Debuggatha — CLAUDE.md

## What is Debuggatha

A senior code reviewer, not a chat wrapper. Debuggatha understands a repository's
context — stack, architecture, and its own stated rules — before it reviews
anything, and every finding it produces traces back to a real source.

Thesis: **"A senior reviewer that understands repository context, applies
explicit criteria, and produces traceable findings."** Not "AI-powered code
review chat." This thesis should be able to guide architecture, UX, and
feature decisions regardless of which model is fashionable next year.

## Status

The original v2.0.0 (VS Code extension, 3 selectable personalities, 4
hardcoded providers, 10-file/100KB cap, never published to Marketplace/Open
VSX, stalled since 2026-02-11) is being redesigned from zero. This document
is the product design produced 2026-07-10, before any code refactor starts,
and **supersedes the original README's positioning**. Treat the old
README/package.json as historical, not as a spec to preserve.

## Implementation status

- **Epic 1 — Repository Intelligence: implemented.** Lives in
  `packages/repository-intelligence` (`@debuggatha/repository-intelligence`,
  alias `@repo-intel`). Produces the immutable `RepositoryContext` — Stack
  Detection, Dependency Context, Documentation Context, Criteria
  Resolution, and Repository Understanding all ship for real, each with
  its own test suite, plus a fingerprint-based cache. See that package's
  own README for the architecture summary, technical decisions, and known
  risks/limitations — don't duplicate that detail here.
- **Epic 2 — Review Engine domain: implemented.** Lives in
  `packages/review-engine` (`@debuggatha/review-engine`, alias
  `@review-engine`). Defines `ReviewRequest`, `ReviewSession` (with an
  enforced lifecycle FSM), `Finding` (evidence and location required to
  construct one), `Evidence`, `Category` (open/extensible), `Severity`
  and `Confidence` (independent, never derived from each other),
  `Recommendation`, `ReviewResult`, and `ReviewSummary`. Independent from
  MCP/CLI/VS Code/providers **and** from Review Packs/Policies (referenced
  only by `{ id }`). See that package's own README for the architecture
  summary, domain model table, technical decisions, and risks.
- **Epic 3 — Knowledge System: implemented.** Lives in
  `packages/knowledge-system` (`@debuggatha/knowledge-system`, alias
  `@knowledge-system`). Defines `SkillDescriptor`, `ReviewPack`/`Rule`/
  `RuleScope`/`KnowledgeEntry`, an injectable `CapabilityRegistry` (+
  `createInMemoryRegistry`), `resolvePolicy` (Rule Resolution — pure,
  fail-closed on missing dependencies, produces a deterministic
  `ReviewPolicy.id` per ADR-0005), `assembleContext` (Context Assembly →
  `SkillContext`), `ReviewPolicy`/`ConflictRecord` (conflicts are recorded
  with fixed precedence, never silently dropped, per ADR-0003), and
  `validateReviewPack`. Depends only on `@debuggatha/repository-intelligence`
  and `@debuggatha/review-engine`, same direction Epic 2 already depends
  on Epic 1. `@debuggatha/review-packs` now ships the real `ReviewPack`
  type plus one trivial `examplePack`; `@debuggatha/policies`'
  `assemblePolicy` now actually calls `resolvePolicy` against a registry
  populated from `review-packs`; `packages/skills/src/review/*`'s
  `reviewArchitecture`/`reviewDiff` had their type imports migrated off
  `@debuggatha/shared` onto `@debuggatha/review-engine` (`Finding`) and
  `@debuggatha/knowledge-system` (`ReviewPolicy`) — both remain typed
  throw-stubs; only their imports changed, not their logic. See that
  package's own README for the architecture summary, domain model table,
  technical decisions, and risks.
- **Epic 4 — Findings Ledger: implemented.** Lives in
  `packages/findings-ledger` (`@debuggatha/findings-ledger`, alias
  `@ledger`). Defines `Ledger`/`LedgerEntry` (immutable — every operation
  returns a new value), the full five-state `FindingLifecycleStatus`
  (open/acknowledged/resolved/dismissed/reopened) as an enforced FSM,
  append-only `HistoryEvent[]`, Finding Identity via
  `computeFindingFingerprint` (file + rule id + content anchor — never
  line number alone), a pluggable `FindingMatcher`,
  `synchronizeReviewResult` (reconciles a `ReviewResult` into the ledger:
  new/matched/auto-resolved/reopened, scoped to what the review actually
  covered), `summarizeLedger`, and deterministic versioned persistence at
  `.debuggatha/ledger.json`. Depends only on
  `@debuggatha/repository-intelligence` and `@debuggatha/review-engine` —
  not on `@debuggatha/knowledge-system`, since it only needs review-engine's
  `ReviewPackReference`/`ReviewPolicyReference` stand-ins, not real pack
  content. See that package's own README for the architecture summary,
  persistence format, technical decisions, and risks.
- **Epic 5 — MCP Server: implemented.** Lives in `packages/mcp`
  (`@debuggatha/mcp`). Thin adapter — every tool handler delegates to
  `@debuggatha/core` via an injected `DomainDeps`, no business logic in
  this package (confirmed: zero reverse imports from any domain package).
  Tools: `review_diff`, `review_files`, `review_workspace`,
  `list_findings`, `get_finding`, `update_finding`, `repository_context`,
  `repository_summary`. Resources: repository context/summary, ledger,
  ledger history. Prompts: `review-flow`, `triage-findings`. Errors are
  sanitized before reaching the client (raw fs paths/stack traces never
  leak — see `errors.ts`).
- **Epic 6 — CLI: implemented, partial.** Lives in `packages/cli`
  (`@debuggatha/cli`). Correctly delegates to `@debuggatha/core` (no
  reimplemented domain logic). `init`, `findings` (list/show/resolve/
  dismiss/reopen/summary), `repository`, `packs`, `doctor` are real.
  Known gaps, not yet closed as of the Epic 16A audit: `doctor` covers
  only 4 of the ~6 checks originally scoped (Node version, git presence,
  `.debuggatha` presence, context-build success — evidence trail and
  policy-conformance checks are still missing). Closed by Epic 16B:
  `review --files` has worked since Epic 11's CLI rewrite (this line
  previously claimed otherwise — stale); `policies` now queries
  `defaultCapabilityRegistry.listPacks()` instead of returning a
  hardcoded single entry; `review`/`findings` now exit with distinct
  codes for a successful run with no findings (`0`), a successful run
  that found active findings (`2`), and an unexpected error (`1`).
- **Epic 6.5 — Audit Closure: implemented.** `docs/PACK_SPEC.md`
  is the canonical Review Pack spec. `@debuggatha/review-packs` now ships
  29 real Foundation Bundle packs (see "Review Packs" below), replacing
  the single example pack referenced above. Of the five architectural
  findings from the audit: Capability Registry, Shared Domain Vocabulary,
  File Review readiness, and Knowledge System extension points are
  resolved; the Git Abstraction (`RepositoryProvider` in
  `packages/core/src/provider.ts`) exists and is used by the CLI, but
  `apps/vscode`'s own git provider does not yet go through it — still
  shells out to git directly.
- **Epic 7 — VS Code Client: implemented, partial.** Lives in
  `apps/vscode`. Thin adapter over `@debuggatha/core`, native VS Code
  components only (tree views, status bar, codicons — no custom webview
  UI). Repository Intelligence view, Findings Ledger view (resolve/
  dismiss/reopen), review commands (workspace/diff/file), settings, and
  a welcome walkthrough all ship. Known gap: "Review Selection" has no
  selection-scoped logic yet and falls back to reviewing the whole file
  (`debuggatha.reviewSelection` in `src/index.ts`) — this is a real TODO,
  not a hidden defect.
- **Epic 8 — User Experience: partial.** Onboarding (VS Code walkthrough,
  `debuggatha doctor`'s actionable diagnostics) and terminology
  consistency are solid. Progressive disclosure (settings/flags are flat,
  no basic/advanced tiering) and some empty-state coverage (CLI has none;
  VS Code has it only via `viewsWelcome`) are not yet addressed.
- **Epic 9 — Hardening: in progress.** Build/typecheck/full test suite
  are green across every package. Weakest test coverage:
  `packages/review-packs` (1 test for 29 pack files — schema/loading is
  covered, individual pack content is not), `apps/vscode` (smoke tests
  only, no mocked-domain adapter tests). `packages/core` now has real
  tests (Epic 11's `pipeline.test.ts`), closing the "0 tests" gap noted
  in the original version of this section.
- **Epic 11 — Review Skills Engine: implemented, partial.**
  `reviewArchitecture`/`reviewDiff`/`reviewFiles` in
  `packages/skills/src/review/*` are no longer stubs — all three now run
  through one shared, deterministic engine
  (`packages/skills/src/engine/run.ts#runReviewSkillsEngine`) that turns
  a resolved `ReviewPolicy` + `SourceUnit[]` into citable `Finding[]`,
  differing only in what source units each scope hands the engine (whole
  files, added diff lines, or the whole tree — see that package's own
  README). A curated `RULE_DETECTORS` registry (partial coverage,
  documented as such) turns specific declarative Rules into real
  line-level checks; Architecture Review additionally surfaces layer
  violations/dependency cycles from `@debuggatha/analysis-engine`
  (Epic 12). Separately, the orchestration sequence itself (build context
  → assemble policy → create request/session → run a Skill → transition
  the session → sync the ledger) — previously hand-rolled identically in
  `@debuggatha/mcp`, `@debuggatha/cli`, and `apps/vscode` — is now one
  function, `executeReview` in `@debuggatha/core`
  (`packages/core/src/pipeline.ts`), that all three adapters call
  through. Known gaps, not yet closed: real per-finding LLM judgment
  (would need MCP sampling wired end-to-end, a transport-level change out
  of this epic's scope — see the skills package README "Technical
  decisions"); Architecture Review does not detect which architectural
  pattern is in use, only runs rule detectors across the tree plus
  Epic 12's layer/cycle findings. (The `javascript`/`typescript` stack
  packs' rules never surviving Rule Resolution, originally noted here as
  an open Epic 1/Epic 6.5 boundary issue, is fixed as of Epic 12.5 — see
  that entry below.) See `packages/skills/README.md` "Known limitations" and
  "Remaining work" for the full list.
- **Epic 12 — Analysis Engine: implemented.** Lives in
  `packages/analysis-engine` (`@debuggatha/analysis-engine`). A
  deterministic, LLM-free repository analysis pipeline: Module Boundaries
  → Dependency Graph → Layer Model → Layer Violations/Cycles → Public API
  Surface → Dead Code → Ownership, plus a separate on-demand Change
  Impact entry point. Every stage consumes the previous stage's output
  (never re-derives it); Module Boundaries prefers a monorepo's own
  `packages/*`/`apps/*` layout over its `src/<name>/*` directory-name
  heuristic fallback. Cached via an injectable, in-memory `AnalysisCache`
  (same discipline as Epic 1's `RepositoryContextCache`), invalidated by
  a directory-listing + file mtime/size fingerprint. Exposed through
  `@debuggatha/core`'s façade like every other epic's package.
  `@debuggatha/skills`' Architecture Review (Epic 11) now consumes this
  package's layer violations/cycles instead of its own bespoke traversal
  — Epic 11's `detectDependencyDirectionViolations` is deleted, replaced
  by `engine/analysis-findings.ts#buildAnalysisFindings`. See
  `packages/analysis-engine/README.md` for the full architecture summary,
  cache strategy, known limitations (name-based Layer Model assignment,
  name-based Dead Code detection, CODEOWNERS-only Ownership Detection,
  non-bundler-aware Public API entrypoint detection), and future analysis
  opportunities.
- **Epic 12.5 — Repository Capability Resolution: implemented.** Lives in
  `packages/repository-intelligence` (`capabilities.ts`/`summary.ts`).
  Fixes the exact limitation Epic 11 and Epic 12's READMEs both
  documented: `RepositoryContext.capabilities` (`Capability[]`) is now
  the canonical, machine-matchable contract Review Pack resolution
  consumes — a flat, additive set of normalized ids (`"typescript"`,
  `"react"`, `"bun"`, ...), each with its own `confidence`/`evidence`/
  `origin`, never collapsed into one mutually-exclusive "stack" label.
  `@debuggatha/knowledge-system`'s `resolvePolicy`/`ruleApplies` now
  matches `requires-language`/`requires-framework` scopes against this
  capability set instead of `StackProfile.languages`/`.frameworks`
  exact-string equality — the `debuggatha/typescript`/
  `debuggatha/javascript` stack packs' rules now actually resolve for
  real repositories, proven end-to-end in
  `packages/core/src/pipeline.test.ts`'s "Epic 12.5 capability fix" test.
  `StackProfile` (Epic 1) is untouched and still exists as the
  display-oriented representation; `summarizeCapabilities` generates a
  human-readable summary *from* the capability model (wired into MCP's
  `repository_summary` tool) rather than the old direction. Backwards
  compatible via migration, not a breaking rewrite:
  `RepositoryContext.capabilities` is a new additive field, and every
  hand-constructed `RepositoryContext` fixture across the monorepo
  (`knowledge-system`, `policies`) was updated to include it. See
  `packages/repository-intelligence/README.md`'s new "Repository
  Capability Resolution" section for the full architecture, and its
  "Risks" section for what's still not covered (ASP.NET Core/`.csproj`
  detection, PHP/Composer scanning living outside `scanStack` proper).
- **Epic 13 — Runtime Engine: implemented.** Lives in
  `packages/runtime-engine` (`@debuggatha/runtime-engine`). A
  provider-agnostic execution layer for semantic (model-powered) review —
  the counterpart to Epic 11's deterministic engine. The load-bearing
  design decision: a `Provider` never returns raw model text to the rest
  of Debuggatha — every execution resolves to a normalized
  `SemanticReviewResult` (`candidates: SemanticFindingCandidate[]`, each
  with evidence/confidence/explanation/a severity hint), via
  `parseSemanticResponse`. Swapping providers is transparent past that
  boundary. Ships production `Provider` implementations for **Ollama**
  (`/api/tags`/`/api/generate`/`/api/show`, NDJSON streaming) and
  **LM Studio** (`/v1/models`/`/v1/chat/completions`, OpenAI-compatible
  SSE streaming), both via an injectable `fetchImpl` for testing without
  a real server. `ProviderRegistry` is injectable/in-memory (same
  discipline as every prior epic's registry); adding a new provider (an
  OpenAI/Anthropic/Gemini-compatible endpoint, a future MCP runtime)
  needs no changes to `engine.ts`. `RuntimeEngine` implements Runtime
  Selection (automatic / explicit provider / explicit model, via
  `resolveRuntime`), streaming (`streamSemanticReview`, exposing the raw
  `AsyncIterable<SemanticStreamEvent>`) and cancellation (every execution
  owns an `AbortController`; an external signal is bridged into it, and
  `cancel()` drives the same one — "avoid orphaned requests"), and
  context budgeting (`estimateTokens`, `budgetSourceUnits`,
  `chunkSourceUnit` — never silently drops a unit, always reports why a
  unit was excluded). Integrated with `@debuggatha/skills` via a new,
  separate, opt-in function, `runSemanticFindings` — deliberately *not*
  wired into `reviewFiles`/`reviewDiff`/`reviewArchitecture` (which stay
  synchronous, matching every existing adapter that calls them
  synchronously today); a caller composes deterministic + semantic
  findings explicitly. See `packages/runtime-engine/README.md` for the
  full architecture, provider abstraction, streaming/cancellation flow,
  context budgeting strategy, and future provider extension points; see
  `packages/skills/README.md`'s "Semantic review (Epic 13)" section for
  the integration side. Explicitly out of scope per the epic: cloud
  authentication, and any OpenAI/Anthropic/Gemini-compatible provider
  implementation itself (extension points only).
- **Epic 14 — Context Intelligence: implemented.** Lives in
  `packages/context-intelligence` (`@debuggatha/context-intelligence`).
  Collects engineering context beyond code — documentation intent,
  development workflow, ownership, project metadata, git context — into
  a structured `ContextItem[]`, never a finding: "it does not produce
  findings, it produces understanding." The load-bearing design decision:
  every `ContextItem` stores an extracted *fact*
  (`{ category: "engineering-convention", rule: "Prefer small
  functions", source: "CONTRIBUTING.md", confidence: "documented" }`),
  never an interpreted conclusion — what that fact *means* for a finding
  stays downstream, in a Review Skill or the Runtime Engine. Three of
  seven sources (`buildConventionItems`, `buildDocumentationItems`,
  `buildCapabilityItems`) never touch the filesystem — they retype data
  `@debuggatha/repository-intelligence` (Epic 1/12.5) already parsed;
  `buildOwnershipItems` reuses `@debuggatha/analysis-engine`'s
  `inferModuleBoundaries`/`detectOwnership` (Epic 12) directly rather
  than reimplementing CODEOWNERS parsing — "do not duplicate Repository
  Intelligence... do not duplicate Analysis Engine." Only `detectWorkflow`
  (GitHub Actions CI trigger extraction, release/commit/branching
  convention markers), `buildProjectMetadataItems` (license/version), and
  `buildGitContextItems` (active/default branch, read directly from
  `.git`'s own plumbing files, no `git` binary shelled out to) read
  anything genuinely new. Every item carries a three-level confidence
  (`documented`/`detected`/`inferred`, never promoted upward) and
  file-level evidence — never a bare claim. Cached via a targeted (not
  full-repository-walk) fingerprint keyed off the input
  `RepositoryContext.generatedAt` plus a small fixed set of files/
  directories this package reads beyond that. Exposed through
  `@debuggatha/core`'s façade; `conventionsToPolicyStatements` is a
  light, optional seam feeding repository conventions into
  `@debuggatha/runtime-engine`'s (Epic 13) `policyStatements` parameter.
  See `packages/context-intelligence/README.md` for the full
  architecture, context/confidence/evidence models, cache strategy, and
  remaining future context opportunities (per-source cache invalidation,
  ADR-specific extraction — the "Repository Memory" subsystem this
  epic's Integration section named as a future consumer now exists, see
  Epic 15 below).
- **Epic 15 — Repository Memory: implemented.** Lives in
  `packages/repository-memory` (`@debuggatha/repository-memory`).
  Long-term, repository-specific engineering knowledge that accumulates
  across review sessions — accepted deviations, known false positives
  (suppressions), architectural exceptions, repository conventions,
  review-history annotations, and engineering decisions. "The objective
  is not to remember conversations. The objective is to remember
  engineering decisions" — no prompts, no generated prose, no raw LLM
  output is ever persisted, only structured items. The load-bearing
  design decision (the epic's own author's addition): memory has a real
  six-state lifecycle, not a write-once drawer —
  `detected -> suggested -> confirmed -> active -> deprecated -> archived`,
  enforced the same way `@debuggatha/findings-ledger`'s
  `FindingLifecycleStatus` already is (`canTransitionMemory`/
  `LEGAL_TRANSITIONS` as the single source of truth). Only `active`
  items are ever consulted when filtering findings; `confirmMemory` is
  the only transition that can ever be driven by a `{ kind: "user" }`
  origin, keeping `"user-confirmed"` confidence honest — "never present
  inferred knowledge as confirmed fact." Suppression/accepted-deviation/
  exception matching reuses `@debuggatha/findings-ledger`'s own
  `computeFindingFingerprint` rather than re-deriving finding identity —
  "enrich the Findings Ledger, don't duplicate it." Wired directly into
  `@debuggatha/core`'s `executeReview` (Epic 11's shared pipeline):
  every review run automatically loads `.debuggatha/memory.json` and
  filters findings through `filterSuppressedFindings` before they reach
  the ledger — no manual intervention required, and a repository with no
  memory file yet behaves exactly as before this epic. Every suppressed
  finding is surfaced via the new `ReviewPipelineOutput.suppressedFindings`,
  never silently hidden. Proven end-to-end in
  `packages/core/src/pipeline.test.ts`'s "Epic 15 Repository Memory
  integration" tests. Persisted at `.debuggatha/memory.json` — same
  directory as the Findings Ledger, deterministic sorted-key JSON,
  versioned/fail-closed like `ledger.json`. See
  `packages/repository-memory/README.md` for the full architecture,
  persistence/confidence models, and remaining future work (user-facing
  inspect/confirm/reject/remove adapter surfaces — CLI/MCP/VS Code — none
  wired yet; automatic `detected` → `suggested` promotion; recurring-
  pattern detection; ADR-to-`decision`-item extraction from Epic 14).
  **(Epic 16B closed the CLI half of the adapter-surface gap — see
  below.)**
- **Epic 16A/16B — Deferred Inventory and Resolution: implemented.**
  16A was a research-only audit (no code changes) producing a complete
  inventory of every deferred item, limitation, and unresolved decision
  across the repository — package READMEs, CLAUDE.md itself, and
  runtime-tested CLI behavior. 16B resolved it against a fixed release-
  oriented priority order (P1 mandatory, low-risk P2, P3 polish,
  remaining P2 left intentionally deferred). See
  `docs/DEFERRED_INVENTORY.md` for the full inventory and disposition of
  every item — the summary: corrected several stale "not yet
  implemented" claims left over from pre-Epic-11 package READMEs
  (review-engine, knowledge-system, findings-ledger, `policies`'s own
  code comment); added a `debuggatha memory` CLI command group
  (list/suggest/confirm/activate/reject/deprecate/archive/remove),
  closing Epic 15's adapter-surface gap; fixed a real pre-existing bug
  where every nested CLI subcommand (`findings list`, etc.) crashed on
  invocation due to a wrong command reference reaching the logger;
  wired the CLI's `policies` command to the real registry instead of a
  hardcoded entry; gave `review` distinct exit codes
  (0/no-findings, 2/findings-detected, 1/error); added five new
  detectors (react/vue/rust/go) as a deliberately modest expansion, not
  full Foundation Bundle coverage; closed the `apps/docs` placeholder's
  turbo build warning. One P1 item — wiring real MCP sampling, the
  documented "default" model path — was **deliberately left deferred**
  despite its priority label, because implementing it would violate
  this resolution pass's own explicit constraints (no new Runtime
  providers, no subsystem redesign); this is flagged as the top item
  for the next roadmap.

## Non-negotiable

MCP is distribution only, not the value proposition. Debuggatha is a
specialist an agent calls — Claude Code writes code, Debuggatha reviews,
different roles — never design or market it as "an MCP server" or "another
AI chat."

## Identity — fixed, not selectable

One personality. No dropdown between tones.

- Voice: direct, precise, never condescending or cruel — a senior reviewer
  who already read the code before opening its mouth.
- Ritual: never opines cold. Always orients first (Core Skills), then states
  its understanding of the project in one line before any judgment.
- Never fakes certainty: if it can't detect something, it says so instead of
  guessing silently.
- Analogy: nobody picks Git's personality. Debuggatha has the same implicit,
  non-selectable identity. Not "choose your witch" — "this is Debuggatha."

## Design principle: never opine without evidence

Every finding must cite at least one of:

- A rule from the repository itself
- A Review Pack
- A language/framework convention
- A recognized standard (OWASP, WCAG, etc.)
- Evidence from the analyzed code

Never "I think...". Always "This finding exists because...". This is what
keeps Debuggatha reading as an engineering tool instead of "another opinionated AI."

## Skill taxonomy

Three tiers. Do not flatten them into one list.

**Core Skills** — always run, deterministic, model-independent. This is why
the product's value survives a model swap (Claude → GPT → Gemini → local):
none of these depend on model judgment quality. **Implemented** in
`@debuggatha/repository-intelligence` as of Epic 1 (see "Implementation
status" above) — every claim below carries an evidence trail in the real
implementation, not just a boolean.

- **Stack Detection** — reads manifests (Cargo.toml, pubspec.yaml,
  package.json, go.mod...); resolves ambiguity by content, not filename
  alone (a Cargo.toml is only "Tauri" if it depends on tauri).
- **Criteria Resolution** — *not* "Criteria Loading." Builds a mental model
  of project criteria from every available signal: docs (ARCHITECTURE.md,
  CONTRIBUTING.md, CLAUDE.md, UX_CRITERIA.md) **and** config surfaces
  (.eslintrc, rustfmt.toml, biome.json, detekt.yml, .editorconfig) **and**
  what manifests imply about how the team works. JSON/INI-shaped configs
  parse into real structured rules; rule-shaped Markdown sections
  structure at heading granularity (the prose under a heading is not
  semantically understood — see the package README's "Risks").
- **Documentation Context** — README, ADRs, PRD, Roadmap → intent, not just
  structure.
- **Repository Understanding** — fallback: asks or discovers what the
  deterministic skills couldn't. Never invents an answer — only raises a
  question when a concrete gap exists.

**Review Skills** — produce judgment, invoked per depth/scope.

- Architecture Review — detects the pattern (Clean Architecture, Hexagonal,
  MVC, MVVM, Feature-First, Layered) and adapts critique to it.
- Security Review, Performance Review, Accessibility Review — catalog grows
  over time, not all at once.

**Analysis Skills** — discover, don't judge. Feed Review Skills and Review
Packs with structural facts.

- Dependency Graph, Module Boundaries, Ownership Detection, Change Impact.

## Review Packs

Not "Knowledge Packs" — the user consumes them to review, not just to know.

```
Review Pack
├── Skills     — capabilities the pack activates
├── Policies   — assembled, citable ruleset for a given review
├── Rules      — atomic, individually citable statements
└── Knowledge  — underlying domain expertise the rules derive from
```

- Stack packs: Rust, Flutter, React, Tauri, Kotlin...
- Concern packs: OWASP, Accessibility, Performance, DX, Release...

A **Review Policy** is the specific combination of a pack's Rules + the
repo's own resolved criteria (via Criteria Resolution), assembled for one
review run. Always inspectable, always citable.

This hierarchy is **implemented** as of Epic 3 in
`@debuggatha/knowledge-system` — `ReviewPack`/`Rule`/`KnowledgeEntry`
types, the `CapabilityRegistry`, `resolvePolicy` (Rule Resolution), and
`ReviewPolicy`/`ConflictRecord` all ship for real. See that package's own
README for the domain model, technical decisions, and risks — don't
duplicate that detail here. As of Epic 6.5, `@debuggatha/review-packs`
ships the real Foundation Bundle: 29 packs spanning languages
(TypeScript, JavaScript, Rust, Go, Kotlin, Dart, PHP), runtime (Bun,
Node.js), frontend (React, Vue, Svelte, Astro, Lit), backend (Express,
Fastify, Hono, Elysia, Laravel, ASP.NET Core), desktop/mobile (Tauri,
Electron, Flutter), and concern packs (OWASP, Accessibility, Performance,
Architecture, DX, Release). Conforms to `docs/PACK_SPEC.md`.
Coverage depth varies by pack — 21 are substantive with multiple
technology-specific rules citing authoritative sources; 8 (`hono`,
`elysia`, `owasp`, `accessibility`, `performance`, `architecture`, `dx`)
are schema-conformant but thin (3-4 rules each) and are the natural next
target for expansion, not a defect.

## Vocabulary

| Don't say | Say | Why |
|---|---|---|
| Prompt | Skill | Not text pasted to a model — something Debuggatha knows how to do. |
| Templates / Knowledge Packs | Review Pack | Consumed to review, not just to "know." |
| Prompt (per run) | Review Policy | Auditable criteria behind a specific finding, not "how we talked to the model this time." |
| Criteria Loading | Criteria Resolution | Not loading markdown — resolving a mental model from every signal available (docs + linter/formatter configs + manifests). |
| Personality (plural) | Identity (singular) | One fixed voice, not a selectable persona. |

## What NOT to do

- Do not bring back a personality selector. One identity, always.
- Do not let a Review Skill run before Core Skills have oriented on the repo.
- Do not ship a finding with no citable source — no source, no finding.
- Do not make any Core Skill depend on a specific model provider.
- Do not market or design around "MCP server" as the pitch — it's plumbing.

Two rules from this section's original v1 revision were deliberately
overridden by later epics, not violated by accident — recorded here so
the reversal is a decision, not drift:
- ~~"Do not build the full Review Pack catalog before v1 validates the
  core hypothesis."~~ Epic 6.5 built the full Foundation Bundle (29
  packs) explicitly, ahead of the original v1 gate.
- ~~"Do not build the VS Code/Open VSX panel before the engine works
  headless via MCP."~~ Both shipped together — Epic 5 (MCP) and Epic 7
  (VS Code) — rather than strictly sequenced.

## v1 scope

The only rule that matters before starting on anything past this:

> The first version must demonstrate that Debuggatha understands a
> repository better than a generic chat.

Ships:

- **Identity**: complete from day one.
- **Core Skills**: all four (Stack Detection, Criteria Resolution,
  Documentation Context, Repository Understanding) — **shipped**, see
  "Implementation status."
- **Review Skills**: Diff Review + Architecture Review only — implemented
  as of Epic 11 (deterministic detector-registry engine, partial
  coverage), see "Implementation status."
- **Review Packs**: originally scoped as 2–3 (React/TS, Kotlin, one
  concern pack); Epic 6.5 shipped the full 29-pack Foundation Bundle
  instead — see "What NOT to do" for why that's a recorded decision, not
  scope creep. Validating against Animoria and Sxnnyside's own repos is
  still outstanding regardless of catalog size.
- **Distribution**: MCP server, sampling-first (uses the host's
  already-configured model — Claude Code, Claude Desktop, Cursor, Copilot
  agent mode). No key management for the default path.

## Deferred (not v1)

- Direct/local model runtime (Ollama, LM Studio) — `debuggatha.runtime`
  exposes a `local` option in VS Code settings, but nothing behind it is
  implemented yet; `mcp` is the only working runtime today

Shipped ahead of this section's original scope (update this list, don't
trust it blindly): the Review Pack catalog (29 packs, Epic 6.5), the
VS Code/Open VSX visual panel (Epic 7), the standalone CLI (Epic 6), and
the Analysis Skills tier (Dependency Graph, Module Boundaries, Ownership
Detection, Change Impact — Epic 12, `@debuggatha/analysis-engine`) all
now exist — this section originally deferred all four past v1, but
implementation moved faster than this document was updated. See
"Implementation status" above for their actual state.

## MCP surface (distribution layer)

**Implemented** as of Epic 5 in `@debuggatha/mcp`. Tool names below match
the shipped implementation, not the earlier draft naming in this section's
prior revision (`resolve_finding` was renamed to `update_finding`;
`set_voice` was not built — depth/voice are still request parameters on
the review tools, not a separate tool).

- `review_diff` — default mode, audits what changed since last commit / a
  base branch.
- `review_files` — explicit paths, no artificial file cap.
- `review_workspace` — full sweep, meant to run once and seed the ledger.
- `list_findings` / `get_finding` / `update_finding` — read/update finding
  state via `@debuggatha/findings-ledger`.
- `repository_context` / `repository_summary` — expose Repository
  Intelligence without running a review.

All three review tools call through to the now-real
`reviewDiff`/`reviewFiles`/`reviewArchitecture` skills (Epic 11, see
"Implementation status") via `@debuggatha/core`'s shared `executeReview`
pipeline — real findings sync into the ledger, subject to that epic's
documented detector-coverage limitations.

## Model strategy

- **Default — sampling**: the server asks the connected host to run
  inference with whatever model it already has configured. Zero keys
  managed by Debuggatha.
- **Opt-in — direct runtime**: local (Ollama, LM Studio) or a user's own
  cloud key, for fixed-model or fully offline use.

## Findings ledger

**Implemented** as of Epic 4 in `@debuggatha/findings-ledger`. Persists
`@debuggatha/review-engine`'s `Finding`s across sessions, layering on what
`Finding` itself deliberately doesn't carry: a stable identity separate
from `Finding.id` (`LedgerEntry.id`, matched via
`computeFindingFingerprint` — file + rule id + a content-anchor hash,
never line number alone), the full five-state lifecycle
(open/acknowledged/resolved/dismissed/reopened, enforced), and an
append-only history of every transition. `synchronizeReviewResult`
reconciles a fresh `ReviewResult` into the ledger — new findings get
entries, still-detected ones refresh silently, findings no longer
detected within the reviewed scope auto-resolve, and previously
resolved/dismissed findings that reappear reopen. Persisted at
`.debuggatha/ledger.json` — deterministic (sorted keys), versioned
(`schemaVersion`, fails closed on an unrecognized one), human-readable.
See that package's own README for the architecture summary, persistence
format, technical decisions, and risks — don't duplicate that detail
here.

Do not confuse this with `@debuggatha/repository-intelligence`'s
`RepositoryContextCache` — that one caches the *repository snapshot*
(stack/deps/docs/criteria) to avoid rescanning, in memory, per process.
This ledger is a *persistent record of review findings* across sessions.
Different subsystem, different lifetime, both deliberately separate.

As of Epic 5-7, all three adapters (MCP, CLI, VS Code) call
`synchronizeReviewResult` on every review run, via `@debuggatha/core`'s
shared `executeReview` pipeline (Epic 11). As of Epic 11 the Review
Skills underneath are real (see "Implementation status"), so this now
synchronizes actual findings, not an empty `ReviewResult` — subject to
that epic's documented detector-coverage limitations.

## Origin

Design produced across a 2026-07 audit + product redesign session:
(1) an objective audit of v2.0.0 as it existed, (2) an MCP-first
repositioning proposal, (3) the identity/skills redesign this file
consolidates, (4) Epic 1 (Repository Intelligence), (5) Epic 2 (Review
Engine domain), (6) Epic 3 (Knowledge System), (7) Epic 4 (Findings
Ledger), (8) Epic 5 (MCP Server), (9) Epic 6 (CLI), (10) Epic 6.5 (Audit
Closure — PACK_SPEC.md + Foundation Bundle), (11) Epic 7 (VS Code
Client), (12) Epic 8 (User Experience, partial), (13) Epic 9 (Hardening,
in progress — this document's Epic 5-9 status was itself the largest gap
found during that pass, corrected 2026-07-11), (14) Epic 11 (Review
Skills Engine — replaced the `reviewArchitecture`/`reviewDiff`/
`reviewFiles` stubs with a real deterministic engine, and deduplicated
the orchestration pipeline previously hand-rolled separately in MCP/CLI/
VS Code into `@debuggatha/core`'s `executeReview`, 2026-07-13), (15)
Epic 12 (Analysis Engine — `@debuggatha/analysis-engine`'s deterministic
Module Boundaries/Dependency Graph/Layer Model/Layer Violations/Cycles/
Public API Surface/Dead Code/Ownership/Change Impact pipeline, consumed
by Epic 11's Architecture Review in place of its own bespoke dependency
traversal, 2026-07-13), (16) Epic 12.5 (Repository Capability
Resolution — replaced `StackProfile`-string matching with the additive
`Capability[]` model in `@debuggatha/repository-intelligence`, fixing
the `javascript`/`typescript` stack-pack resolution bug Epic 11 and
Epic 12 both surfaced but didn't fix, 2026-07-13), (17) Epic 13
(Runtime Engine — `@debuggatha/runtime-engine`'s provider-agnostic
semantic execution layer, with production Ollama/LM Studio providers,
runtime selection, streaming, cancellation, and context budgeting, all
behind a normalized `SemanticReviewResult` contract rather than raw
model text; integrated into `@debuggatha/skills` via the new opt-in
`runSemanticFindings`, 2026-07-13), (18) Epic 14 (Context
Intelligence — `@debuggatha/context-intelligence`'s structured,
fact-not-interpretation `ContextItem` model over documentation intent,
development workflow, ownership, project metadata, and git context,
built by retyping Epic 1/12/12.5's already-parsed data plus three
genuinely new sources, never producing findings, 2026-07-13), (19)
Epic 15 (Repository Memory — `@debuggatha/repository-memory`'s
six-state lifecycle (`detected` -> `suggested` -> `confirmed` ->
`active` -> `deprecated` -> `archived`) over accepted deviations,
suppressions, exceptions, conventions, review-history notes, and
decisions, wired automatically into `@debuggatha/core`'s `executeReview`
so only user-confirmed, active memory ever suppresses a finding,
2026-07-13). Treat this file as the living summary — update it directly
when a decision here changes, rather than accumulating a new document
per session.
