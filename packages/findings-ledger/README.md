# @debuggatha/findings-ledger

Epic 4. The historical record of every finding ever produced for a
repository — persistence, traceability, and lifecycle management across
many review sessions. See [CLAUDE.md](../../CLAUDE.md) at the repo root
for the product decisions this package encodes.

This package is **not a database**. It does not know what MCP, the CLI,
or VS Code are, does not talk to an AI provider, and does not implement
cloud sync, multi-user collaboration, GitHub integration, PR annotations,
remote storage, or telemetry — those are explicitly future epics. It
depends only on `@debuggatha/repository-intelligence` (Epic 1, for
`RepositoryContext`) and `@debuggatha/review-engine` (Epic 2, for
`Finding`, `ReviewResult`, `Severity`, `Category`) — never on
`@debuggatha/knowledge-system` (Epic 3), since nothing here needs a
`ReviewPolicy`'s actual content, only the `ReviewPackReference`/
`ReviewPolicyReference` stand-ins review-engine already carries.

## Architecture summary

```text
ReviewResult (Epic 2) ──┐
                         ├─► synchronizeReviewResult(ledger, result, context, scope)
RepositoryContext (Epic 1)┘         │
                                     │ for each Finding: defaultFindingMatcher
                                     │   -> new            => createLedgerEntry (status: open)
                                     │   -> matched, active => refreshLedgerEntry (no history event)
                                     │   -> matched, closed => transitionFinding(...,"reopened")
                                     │ for each untouched *active* entry within `scope`:
                                     │   => transitionFinding(...,"resolved")
                                     ▼
                                  Ledger { entries: LedgerEntry[] }  ── frozen
                                     │
                    ┌────────────────┼────────────────┐
                    ▼                ▼                ▼
            updateFindingStatus  summarizeLedger  serializeLedger / saveLedger
            (manual transitions)  (LedgerSummary)   → .debuggatha/ledger.json
```

Every operation returns a **new** `Ledger` — the same immutability
discipline `RepositoryContext` (Epic 1) and `ReviewPolicy` (Epic 3)
already established. Nothing in this package mutates a `Ledger`,
`LedgerEntry`, or `HistoryEvent` in place.

## Domain model overview

| Type | Role |
|---|---|
| `Ledger` | `{ schemaVersion, repositoryRoot, entries, createdAt, updatedAt }` — one repository's complete history. |
| `LedgerEntry` | One tracked issue across its lifetime: stable `id` (distinct from `latestFinding.id`, which review-engine regenerates every run), `fingerprint`, `status`, append-only `history`, `latestFinding`, `review`. |
| `FindingFingerprint` | `{ file, ruleId, category, contentAnchor }` — Finding Identity (epic §2). Never keys on line number alone. |
| `FindingLifecycleStatus` | `open \| acknowledged \| resolved \| dismissed \| reopened` — enforced by `canTransitionFinding`, the same explicit-transition-table pattern review-engine's `ReviewLifecycleStatus` uses. |
| `HistoryEvent` | `{ timestamp, previousState, newState, origin, comment }` — one immutable record of a transition. `Ledger`/`LedgerEntry` history is append-only; nothing ever rewrites a past event. |
| `TransitionOrigin` | `{ kind: "review"; sessionId }` or `{ kind: "manual"; actor }` — what triggered a transition, without importing anything from an adapter package. |
| `ReviewAssociation` | `{ createdBySessionId, lastUpdatedBySessionId, repositorySnapshot, appliedPolicy, appliedPacks }` — complete traceability (epic §5). |
| `RepositorySnapshotRef` | `{ generatedAt, fingerprint }` — a lightweight reference to the repository state a finding was produced against, not the whole embedded `RepositoryContext`. |
| `FindingMatcher` | `(candidate, candidateEntries) => MatchOutcome` — pluggable (epic §6 "design for future improvements"); `defaultFindingMatcher` is the v1 strategy. |
| `LedgerSummary` | `{ countsByStatus, activeBySeverity, activeByCategory }` — repository-level status (epic §7), reusable by any future adapter. |

## Implemented capabilities

- **Ledger** — `Ledger`, `createEmptyLedger`. Belongs to one repository (`repositoryRoot`); accumulates `LedgerEntry[]` across many review sessions.
- **Finding Identity** — `computeFindingFingerprint`: `(file, ruleId | category, contentAnchor)`, extracted from `Finding.evidence` (the `review-pack-rule`/`criteria` variants for `ruleId`, the `code` variant's `excerpt` — hashed — for `contentAnchor`). Line number is never part of identity.
- **Finding Lifecycle** — all five states, `canTransitionFinding` (explicit table), `transitionFinding` (pure, throws on an illegal transition).
- **History** — `HistoryEvent[]`, append-only; every transition (`transitionFinding`, `updateFindingStatus`, auto-resolve, reopen) adds exactly one event and never edits a prior one.
- **Review Association** — `associationFor`, `snapshotRefFor`: every entry knows which session created it, which last confirmed it, the repository state (by content fingerprint, not embedded), and the applied policy/packs.
- **Finding Matching** — `defaultFindingMatcher`, pluggable via `FindingMatcher`.
- **Repository Status** — `summarizeLedger`.
- **Persistence** — `serializeLedger`/`deserializeLedger` (pure, deterministic, versioned via `schemaVersion`, fails closed on an unrecognized version) and `loadLedger`/`saveLedger`/`ledgerFilePath` (thin fs wrappers, `.debuggatha/ledger.json`).
- **Public API** — `listEntries`, `getEntry`, `getHistory`, `updateFindingStatus` (the only sanctioned way to change a status — "adapters should never manipulate persistence directly"), `synchronizeReviewResult`.

## Deferred (explicitly out of scope for this epic)

- Cloud synchronization, multi-user collaboration, GitHub integration, PR annotations, remote storage, telemetry — epic's explicit "Out of Scope" list.
- ~~Any consumer: MCP, CLI, VS Code wiring — nothing here is called by anything yet.~~ **Resolved — Epics 5-7 wired all three adapters, and Epic 11's shared `executeReview` pipeline (`@debuggatha/core`) now calls `synchronizeReviewResult` on every review run.**
- A smarter default `FindingMatcher` (AST diffing, embedding similarity) — the interface exists for this; the implementation doesn't need to yet.
- Pruning/archival of very old resolved/dismissed entries — the ledger only grows in v1.
- A real schema migration (`migrateLedger` has the shape but no logic — only `CURRENT_SCHEMA_VERSION` is accepted today).

## Technical decisions

- **`SyncScope` is supplied by the caller, never inferred from `ReviewRequest.scope`.** Knowing which files a `"diff"` request actually touched requires git knowledge this package deliberately doesn't have — Repository Intelligence doesn't provide it either (it scans state, not history). `synchronizeReviewResult` takes an explicit `{ kind: "workspace" }` or `{ kind: "files"; files }` instead, so a future orchestrator (which *does* know the diff) supplies accurate scope rather than this package guessing.
- **Matching candidates are every ledger entry, not just active ones.** A resolved or dismissed entry must still be matchable — matching one is exactly what triggers reopening it. Only the separate auto-resolve pass is restricted to active (open/acknowledged/reopened) entries.
- **A multi-location `Finding` is fingerprinted on its first location only.** Most findings are single-location; a finding spanning several files (e.g. an architecture violation) is matched on the first one. Documented, not silently assumed — see Risks.
- **`LedgerEntry.id` is a fresh random id, not derived from the fingerprint.** A content-derived id would make two coincidentally-identical fingerprints collide into one entry even if they're genuinely different issues (e.g. the exact same rule violated in two structurally identical files that happen to share a fingerprint's non-file fields — this can't happen here since `file` is part of the fingerprint, but the same caution applies generally); a random id plus fingerprint-based *matching* keeps identity assignment and matching as two separate, independently-reasoned-about concerns.
- **`updateFindingStatus` enforces the same FSM as an automated transition — no privileged "manual override" path.** A human can't jump straight from `open` to `reopened`; the same `canTransitionFinding` table governs both origins. `TransitionOrigin` records *who*, not a different set of *what's allowed*.
- **No new dependency for persistence.** `.debuggatha/ledger.json` is plain `JSON.stringify`/`JSON.parse` with a hand-rolled deterministic key-sorter (shared technique with `@debuggatha/knowledge-system`'s policy-id hashing) — no schema-validation library, matching the minimal-dependency discipline every prior epic in this monorepo has kept.
- **`countsByStatus` covers the full lifecycle; `activeBySeverity`/`activeByCategory` cover only open/acknowledged/reopened.** A resolved or dismissed finding's severity/category isn't actionable anymore — including it in the breakdown a CLI/MCP/VS Code adapter shows a user would misrepresent "what needs attention right now."

## Persistence format overview

`.debuggatha/ledger.json` — one JSON file, UTF-8, 2-space indented,
trailing newline, top-level keys sorted alphabetically (and every nested
object's keys too) so the exact same `Ledger` value always serializes to
the exact same bytes. Top-level shape:

```json
{
  "schemaVersion": 1,
  "repositoryRoot": "/absolute/path/to/repo",
  "entries": [
    {
      "id": "…",
      "fingerprint": { "file": "…", "ruleId": "…", "category": "…", "contentAnchor": "…" },
      "status": "open",
      "history": [
        { "timestamp": "…", "previousState": null, "newState": "open", "origin": { "kind": "review", "sessionId": "…" }, "comment": null }
      ],
      "latestFinding": { "...": "the full Finding object, as review-engine defines it" },
      "review": {
        "createdBySessionId": "…",
        "lastUpdatedBySessionId": "…",
        "repositorySnapshot": { "generatedAt": "…", "fingerprint": "…" },
        "appliedPolicy": null,
        "appliedPacks": []
      }
    }
  ],
  "createdAt": "…",
  "updatedAt": "…"
}
```

A single file (not one-file-per-entry) was chosen for v1: simpler to
load/save atomically, and nothing in this epic asks for git-diff
friendliness on the persisted file. One-file-per-entry (better for
merge-conflict avoidance in a team setting) is a reasonable future
change if that becomes a real pain point — deferred, not designed away.

## Risks

- **A multi-file finding's identity ignores every location after the first.** If two different multi-file findings happen to share the same first `(file, ruleId)` pair but differ elsewhere, they'll incorrectly match. This was written when nothing produced real findings yet; as of Epic 11, real findings do exist, so this risk is now live rather than theoretical — still not commonly hit in practice (most findings from `packages/skills`' detector engine are single-location), but worth re-assessing if multi-location findings (e.g. Epic 12's dependency-cycle findings, which do cite several files) start showing up frequently in ledger entries.
- **The default matcher's `(file, category)` fallback (when no `ruleId` is available) is coarse.** Two unrelated findings in the same file and category with no rule evidence will match each other, potentially merging distinct issues into one ledger entry. Acceptable because most findings are expected to carry rule-shaped evidence (`review-pack-rule` or `criteria`) once Review Packs ship real content; this fallback only matters for Core-Skill-produced findings with no rule behind them.
- **`RepositorySnapshotRef.fingerprint` covers `stack`/`dependencies`/`documentation`/`criteria` but not `generatedAt`/`hasGit`.** Two scans of an unchanged repository at different times produce the same fingerprint (intentional — the fingerprint is about *content*, not *when it was scanned*) but this means `repositorySnapshot.fingerprint` alone can't be used to detect "this is definitely the exact same scan," only "this is the same repository content."
- **No enforcement that `latestFinding.id` values are ever cleaned up or deduplicated across history.** Every refresh replaces `latestFinding` wholesale; the ledger does not retain *every* `Finding` object ever produced for an entry, only the most recent one plus the `HistoryEvent` trail of status changes. A full finding-by-finding audit trail (not just status transitions) would need a different, richer history shape — deferred as speculative until a real use case asks for it.
- **`migrateLedger` is a placeholder with no actual migration logic.** The very first schema change will need real code added there; today it only recognizes `CURRENT_SCHEMA_VERSION` and fails closed on anything else, which is correct but untested against a real migration scenario.
