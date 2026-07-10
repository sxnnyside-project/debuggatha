# @debuggatha/review-engine

Epic 2. Defines the review domain: what a request, a session, a finding,
and a result *are* — independent of who or what produces them. See
[CLAUDE.md](../../CLAUDE.md) at the repo root for the product decisions
this package encodes.

This package does **not** talk to an AI provider, does not know what MCP
or the CLI are, and does not implement Review Packs or Review Policies —
it only references them by id. Its job is to transform a request plus a
`RepositoryContext` (from `@debuggatha/repository-intelligence`, Epic 1)
into structured domain objects that *any* review source — an LLM, a
deterministic analyzer, a custom skill, or a human — can populate the
same way.

## Architecture summary

```text
ReviewRequest ──┐
                ├─► createReviewSession ──► ReviewSession (status: "requested")
RepositoryContext ┘                              │
                                    transitionSession (enforced FSM)
                                                  │
                                     "requested" → "prepared" → "running"
                                                  │
                                     ┌────────────┼────────────┐
                                     ▼            ▼            ▼
                                "completed"   "failed"    "cancelled"
                                     │
                                     ▼
                            createReviewResult ──► ReviewResult
                                                    (summary + findings + recommendations)
```

Findings are constructed through `createFinding`, which enforces the
domain's two non-negotiable invariants — at least one location, at least
one piece of evidence — before a `Finding` can exist at all. A
`ReviewResult` can only be built from a session whose status is
`"completed"`; `createReviewResult` throws otherwise. Both invariants are
runtime checks, not just types, because a domain object that violates
CLAUDE.md's "never opine without evidence" principle should be
impossible to construct, not just discouraged.

## Domain model overview

| Type | Role |
|---|---|
| `ReviewRequest` | One unified request shape for Diff/File/Workspace review — discriminated by `scope.kind`, not three separate types. |
| `ReviewSession` | The traceable process record: id, timestamps, the `RepositoryContext` it ran against, selected policy/pack references, lifecycle `status`, `execution` metadata. |
| `ReviewLifecycleStatus` | `requested → prepared → running → (completed \| failed \| cancelled)`, enforced by `canTransition`/`transitionSession` — nothing else in the codebase should encode transition logic. |
| `ReviewSource` | Extensibility point (`ai-provider` \| `deterministic-analyzer` \| `custom-skill` \| `human-review`) — the domain never assumes AI is involved. |
| `Finding` | First-class object: title, explanation, `Severity`, `Confidence` (independent of each other), `Category` (open string), `locations`, `evidence`, `appliedPolicy`/`originatingPack` references, `recommendations`. |
| `Evidence` | Machine-readable discriminated union (`documentation` \| `criteria` \| `review-pack-rule` \| `framework-convention` \| `language-convention` \| `code`). |
| `Recommendation` | Structured fix, not a prompt — `action` + `summary` + optional target location. |
| `ReviewResult` | The structured deliverable: `summary` + `findings` + result-level `recommendations`, linked to its session by `sessionId` rather than embedded in it. |
| `ReviewSummary` | Aggregate counts by severity/category, applied packs/policies, duration — computed once (`summarizeFindings`), reused by every consumer. |
| `ReviewPackReference` (`{ id, version }`) / `ReviewPolicyReference` (`{ id }`) | Stand-ins — this package never imports `@debuggatha/review-packs` or `@debuggatha/policies`. A pack reference carries a real `version` field (Epic 3, `docs/adr/0002`); a policy reference doesn't need one — a policy is identified entirely by its own deterministic id (`docs/adr/0005`). |

## Implemented capabilities

All eleven from the epic: unified Review Requests, traceable Review
Sessions, structured Review Results, first-class Findings, machine-
readable Evidence, an open Category system, independent Severity and
Confidence, structured Recommendations, reusable Review Summaries, an
enforced Review Lifecycle, and source extensibility (`ReviewSource`
covers AI/deterministic/custom-skill/human equally).

## Deferred (explicitly out of scope for this epic)

- Anything that actually *runs* a review — LLM providers, deterministic
  analyzers, `packages/skills`' `reviewArchitecture`/`reviewDiff` stubs
  are untouched and still throw.
- `@debuggatha/review-packs` and `@debuggatha/policies` content/logic —
  this package only references them by id.
- MCP server, CLI, VS Code wiring.
- Prompt generation / AI communication of any kind.
- Convenience lifecycle wrappers (`prepareSession`, `startSession`, ...)
  — `transitionSession` is the one primitive; named wrappers are easy to
  add later and weren't worth the extra API surface now.

## Technical decisions

- **`affectedFiles` is derived, not stored.** The epic lists "affected
  files" and "affected lines" as separate Finding fields; storing both
  risks them drifting out of sync. `Finding.locations: FindingLocation[]`
  (file + optional line range) is the single source of truth; `affectedFiles()`
  is a pure function over it.
- **`ReviewSession` and `ReviewResult` are separate objects, linked by
  `sessionId`**, not one embedding the other. A session is process
  bookkeeping (lifecycle, timestamps, what was selected); a result is the
  deliverable (findings, summary). Consumers can list sessions without
  loading every result, or fetch a result without full session history.
- **`Finding.recommendations` and `ReviewResult.recommendations` are both
  real, not a duplication.** A finding's recommendations are specific
  fixes for that finding; a result's recommendations are review-level
  (e.g. a pattern spanning several findings, like "3 findings relate to
  inconsistent error handling — consider a repo-wide convention").
- **`Category` is `KnownCategory | (string & {})`**, not a closed union —
  "should remain extensible" (epic §6) means adding a category can't be a
  breaking type change. The `string & {}` trick keeps editor autocomplete
  for the known ones.
- **`Severity` and `Confidence` are unrelated types**, not two ends of one
  scale, and no function converts between them — enforced by having no
  such function exist, per epic §7 ("never derive one from the other").
- **`ReviewDepth` is redefined locally**, not imported from
  `@debuggatha/shared` (which already has a `ReviewDepth`). Depending on
  `shared` here would also pull its legacy `Finding`/`Severity`/
  `ReviewPolicy` types into scope, which this epic's richer versions are
  meant to supersede — see Risks.
- **All construction is validated, all transitions are pure.**
  `createFinding`/`createReviewRequest`/`createReviewResult` throw on
  invalid input instead of returning a partially-valid object;
  `transitionSession` returns a new session rather than mutating,
  matching the immutability discipline `RepositoryContext` established in
  Epic 1.

## Risks

- **`@debuggatha/shared` now has two competing, unreconciled domain
  vocabularies.** Its original `Finding`, `Severity`, `Category`,
  `ReviewPolicy`, `PolicyRule`, `ReviewDepth`, `ReviewVoice` types are
  still referenced by `packages/skills`' stub signatures, `packages/policies`,
  and `packages/review-packs` — none of which this epic touched, since
  Review Packs/Policies/Review Skills were explicitly out of scope. When
  those epics are implemented, they will need to migrate onto
  `@debuggatha/review-engine`'s richer types instead, and `shared`'s
  overlapping types should be removed at that point. Until then, a reader
  encountering `Finding` in `shared` versus `Finding` in `review-engine`
  needs to know which one is current (this one).
- **No cross-session/result persistence.** Sessions and Results are
  plain in-memory objects; nothing here writes them anywhere. The
  "findings ledger" concept in CLAUDE.md is a distinct, still-unbuilt
  concern that would consume `ReviewResult` objects.
- **`transitionSession`'s `patch` argument is a blunt instrument.**
  Any caller can set `execution.error` while transitioning to
  `"completed"`, or omit `error` while transitioning to `"failed"` — the
  type system doesn't tie specific `ExecutionMetadata` fields to specific
  target statuses. A stricter version could require an `error` string
  when `to === "failed"`; left as-is to avoid speculative complexity
  before real callers exist.
- **`ReviewScope`'s `"diff"` variant has no validation.** Unlike `"files"`
  (which requires at least one path), an empty or malformed `base` ref
  isn't checked — there's nothing to validate yet since this package
  never resolves a scope against a real repository (that's a future
  epic's job, likely `@debuggatha/core` once it does more than re-export).
