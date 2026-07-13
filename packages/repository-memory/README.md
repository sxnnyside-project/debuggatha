# @debuggatha/repository-memory

Epic 15. Long-term, repository-specific engineering knowledge that
accumulates across review sessions — accepted deviations, known false
positives, architectural exceptions, repository conventions, review
history annotations, and engineering decisions. See
[CLAUDE.md](../../CLAUDE.md) at the repo root.

**Not a conversation history.** No prompts, no generated prose, no raw
LLM output is ever persisted — only structured engineering knowledge
("the objective is not to remember conversations. The objective is to
remember engineering decisions").

## Memory architecture

```text
detected → suggested → confirmed → active → deprecated → archived
```

Mirrors `@debuggatha/findings-ledger`'s own enforced-lifecycle pattern
(`canTransitionMemory`/`LEGAL_TRANSITIONS` is the single source of truth
for legal transitions, same as that package's `canTransitionFinding`),
extended with the six-state lifecycle the epic's own author called out
as a key design decision:

- **`detected`** — noticed automatically, nobody has looked at it.
- **`suggested`** — surfaced to a user ("Debuggatha thinks this is worth
  remembering").
- **`confirmed`** — a user said yes (only `confirmMemory`, gated on a
  `{ kind: "user" }` origin, can produce this transition).
- **`active`** — actually influences reviews (see "Review enrichment"
  below) — the *only* status `filterSuppressedFindings` ever consults.
- **`deprecated`** — stopped influencing reviews, kept for
  re-confirmation (e.g. the repository changed enough that the item
  might no longer hold).
- **`archived`** — terminal. Kept for audit, never consulted again.

`reject` (a `detected`/`suggested` item a user declines) goes straight to
`archived` — it was never true, so there's nothing to deprecate.
`archiveMemory` reaches `archived` from any non-archived state.
`removeMemoryItem` is a distinct, harder action: it deletes the item from
the store entirely, rather than keeping an archived record — offered
because "stop remembering this" and "this was true but no longer
applies, keep the history" are different user intents.

## Persistence model

`.debuggatha/memory.json` — same directory the Findings Ledger already
uses. Deterministic (sorted-key JSON, same discipline as `ledger.json`),
versioned (`schemaVersion`, fails closed on an unrecognized version —
`persistence/serialize.ts#migrateMemoryStore` is where a real migration
step gets added when the schema actually changes), human-readable and
diffable in a PR.

## Confidence model

Three levels (`MemoryConfidence`), deliberately distinct from
`@debuggatha/context-intelligence`'s own three-level scale (Epic 14):
memory is about engineering *decisions*, not just extracted facts.

- **`inferred`** — a deterministic pass's best guess.
- **`documented`** — backed by something already written down (a
  CONTRIBUTING.md rule, a CODEOWNERS entry).
- **`user-confirmed`** — a human explicitly said yes. Never produced
  automatically — the only path to it is `confirmMemory`, which requires
  a `{ kind: "user" }` origin, never a `{ kind: "detection" }` one.

## Memory items and Review enrichment

Six categories (`MemoryCategory`), each a distinct interface:
`accepted-deviation`, `suppression`, `exception`, `convention`,
`review-history`, `decision`. Every item carries `id`, `status`,
`confidence`, `history: MemoryHistoryEvent[]`, and — required, not
optional — `rationale` and `evidence: MemoryEvidence[]`: "users should
always understand why Debuggatha remembers something."

`suppression`/`accepted-deviation`/`exception` items match a
`Finding` the same way the Findings Ledger recognizes "the same finding"
— `matching.ts` reuses `@debuggatha/findings-ledger`'s own
`computeFindingFingerprint` rather than re-deriving finding identity
("Repository Memory should enrich the Findings Ledger. It should not
duplicate it."). `review-history` items link to a real `LedgerEntry.id`
by reference — this package never stores its own copy of a finding's
detection history.

## Integration — automatic, no manual intervention

`filterSuppressedFindings(findings, store)` is wired directly into
`@debuggatha/core`'s `executeReview` (Epic 11's shared pipeline): every
review run loads `.debuggatha/memory.json` (an empty store when none
exists yet — same "no file yet is not an error" discipline as
`loadLedger`) and filters findings through it *before* they reach the
Findings Ledger, automatically, with zero required changes to any
caller. Only `active` memory items are ever consulted — a
`detected`/`suggested` suppression a user hasn't confirmed must never
silently hide a real finding ("never present inferred knowledge as
confirmed fact"). Every suppressed finding is reported in
`ReviewPipelineOutput.suppressedFindings`, never just dropped —
"suppression should remain explicit and traceable... avoid hidden ignore
lists." Proven end-to-end in
`packages/core/src/pipeline.test.ts`'s "Epic 15 Repository Memory
integration" tests (one showing an `active` suppression actually
suppressing, one showing a merely-`detected` one does *not*).

## Migration strategy

`schemaVersion` on every persisted store; `deserializeMemoryStore` fails
closed (a clear error naming the unsupported version) rather than
guessing at an unrecognized shape — the same discipline
`@debuggatha/findings-ledger`'s own `deserializeLedger` established.
`migrateMemoryStore` is currently a no-op past version-equality check;
it's where a real migration step gets added the first time the schema
actually changes.

## Testing

33 tests: lifecycle transitions (full path, illegal skips, terminal
`archived`), item creation/transitions (always starts at `detected`,
pure/non-mutating, requires rationale+evidence), persistence
(round-trip, deterministic serialization, schema-version fail-closed),
matching (suppression/accepted-deviation/exception, scoped and
repository-wide), filtering (only `active` items ever suppress; every
other status is a no-op), and the full API surface (confirm/reject/
deprecate/reactivate/archive/remove, origin enforcement on `confirmMemory`,
purity). Plus two integration tests in `@debuggatha/core`'s own
`pipeline.test.ts` proving the automatic wiring end-to-end.

## Future expansion opportunities

- **User-facing inspect/confirm/reject/remove surfaces** — this package
  exposes the API (`api.ts`); no CLI command, MCP tool, or VS Code UI
  calls it yet. That adapter wiring is explicitly out of this epic's
  scope (same "domain package first, adapter wiring later" sequencing
  every prior epic has followed).
- **Automatic `detected` → `suggested` promotion** — nothing yet
  automatically surfaces a `detected` item for user review; today
  `suggestMemory` must be called explicitly by whatever detects it.
- **Recurring-pattern detection** — the epic names "recurring findings"
  as a memory model concept; this package can *store* a
  `review-history` note referencing recurrence, but nothing yet
  automatically detects a finding recurring across N review runs and
  creates one.
- **Repository-decision extraction from documentation** — `DecisionItem`
  exists as a category, but nothing yet automatically derives one from
  `@debuggatha/context-intelligence`'s documentation items (e.g. an ADR
  becoming a `decision` memory item) — a natural follow-on integration
  between Epic 14 and Epic 15.
