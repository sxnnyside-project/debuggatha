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
- **Not started:** anything that actually *runs* a review — real Review
  Pack content (Rust, React/TS, Kotlin, OWASP...) and the logic inside
  `reviewArchitecture`/`reviewDiff` are both still deferred to a later
  epic. Nothing yet calls `synchronizeReviewResult` — no MCP/CLI/VS Code
  adapter exists to produce a `ReviewResult` in the first place.
- Everything else in "Deferred (not v1)" below is still deferred.

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
duplicate that detail here. Real Stack/Concern pack *content* (the actual
Rust/React/Kotlin/OWASP rule catalog) is still deferred to a later epic;
`@debuggatha/review-packs` today only ships the schema plus one trivial
example pack proving the pipeline works end to end.

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
- Do not build the full Review Pack catalog before v1 validates the core
  hypothesis.
- Do not build the VS Code/Open VSX panel before the engine works headless
  via MCP.
- Do not make any Core Skill depend on a specific model provider.
- Do not market or design around "MCP server" as the pitch — it's plumbing.

## v1 scope

The only rule that matters before starting on anything past this:

> The first version must demonstrate that Debuggatha understands a
> repository better than a generic chat.

Ships:

- **Identity**: complete from day one.
- **Core Skills**: all four (Stack Detection, Criteria Resolution,
  Documentation Context, Repository Understanding) — **shipped**, see
  "Implementation status."
- **Review Skills**: Diff Review + Architecture Review only.
- **Review Packs**: 2–3, anchored in Sxnnyside's own stack — React/TS Pack,
  Kotlin Pack, plus one concern pack (OWASP or DX). Validate against
  Animoria and Sxnnyside's own repos before asking a stranger to trust it.
- **Distribution**: MCP server, sampling-first (uses the host's
  already-configured model — Claude Code, Claude Desktop, Cursor, Copilot
  agent mode). No key management for the default path.

## Deferred (not v1)

- Full Review Pack catalog (Flutter, Tauri, Accessibility, Performance,
  Release packs, etc.)
- Analysis Skills tier entirely (Dependency Graph, Module Boundaries,
  Ownership Detection, Change Impact)
- VS Code / Open VSX visual panel — always a client of the engine, never
  built first
- Standalone CLI for CI usage
- Direct/local model runtime (Ollama, LM Studio) — opt-in, advanced, after
  sampling-first is proven

## MCP surface (distribution layer — reference only)

- `review_diff` — default mode, audits what changed since last commit / a
  base branch.
- `review_files` — explicit paths, no artificial file cap.
- `review_workspace` — full sweep, meant to run once and seed the ledger.
- `list_findings` / `resolve_finding` — read/update finding state. The
  ledger these need is implemented (`@debuggatha/findings-ledger`, Epic 4:
  `listEntries`/`updateFindingStatus`) — what's still missing is the MCP
  tool itself calling them, since no MCP server exists yet.
- `set_voice` — **not** a personality selector. Adjusts `depth`
  (quick/full/architectural) and `voice` (direct/teaching/formal) as call
  parameters, not branded personas.

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

Nothing calls `synchronizeReviewResult` yet — no MCP/CLI/VS Code adapter
exists to produce a `ReviewResult` in the first place. The ledger is
ready to be driven; it doesn't drive anything itself.

## Origin

Design produced across a 2026-07 audit + product redesign session:
(1) an objective audit of v2.0.0 as it existed, (2) an MCP-first
repositioning proposal, (3) the identity/skills redesign this file
consolidates, (4) Epic 1 (Repository Intelligence), (5) Epic 2 (Review
Engine domain), (6) Epic 3 (Knowledge System), (7) Epic 4 (Findings
Ledger). Treat this file as the living summary — update it directly when
a decision here changes, rather than accumulating a new document per
session.
