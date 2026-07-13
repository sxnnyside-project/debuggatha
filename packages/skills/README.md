# @debuggatha/skills

Epic 11. The first production-ready Review Skills Engine. Replaces the
throw/empty-array stubs `reviewArchitecture`/`reviewDiff`/`reviewFiles`
had carried since Epic 3 with a real, unified pipeline: one deterministic
engine that turns a resolved `ReviewPolicy` (`@debuggatha/knowledge-system`)
plus a set of source units into citable `Finding[]`
(`@debuggatha/review-engine`). See [CLAUDE.md](../../CLAUDE.md) at the
repo root.

## Architecture summary

```text
reviewArchitecture(rootDir, policy)  ─┐
reviewDiff(diff, policy)             ─┼─→ SourceUnit[]  (per-scope: whole tree / added diff lines / named files)
reviewFiles(paths, policy)           ─┘        │
                                                ▼
                                  runReviewSkillsEngine(policy, units)
                                                │  for each policy.rules[i]:
                                                │    detector = detectorFor(rule.id)
                                                │    for each unit: run detector, turn hits into Finding[]
                                                ▼
                                            Finding[]
```

The three public functions differ only in *what* `SourceUnit[]` they hand
the shared engine — exactly "the review scope changes, the review engine
does not":

- **File Review** (`reviewFiles`) reads each named file whole
  (`engine/read-files.ts#readSourceUnits`).
- **Diff Review** (`reviewDiff`) parses a unified diff
  (`engine/diff-parser.ts#parseUnifiedDiff`) and hands the engine *only
  the added lines*, each mapped back to its real file line number — this
  is what keeps a diff review from turning into a full workspace sweep.
- **Architecture Review** (`reviewArchitecture`) walks the whole tree
  (`engine/read-files.ts#walkSourceUnits`, skipping `node_modules`/`dist`/
  `.git`/etc.) and additionally turns `@debuggatha/analysis-engine`'s
  (Epic 12) layer violations and dependency cycles into `Finding`s
  (`engine/analysis-findings.ts#buildAnalysisFindings`), citing
  `framework-convention` evidence rather than a Review Pack rule. This
  Skill does not re-walk the dependency graph itself — it consumes the
  Analysis Engine's own `runAnalysis` output (Epic 12: "avoid duplicated
  graph traversals"). Epic 11 originally shipped a bespoke
  `detectDependencyDirectionViolations` doing this walk itself; it's
  deleted now that the Analysis Engine exists.

`runReviewSkillsEngine` (`engine/run.ts`) is the one place a `Finding` gets
built: every hit cites `code` evidence (the exact excerpt/line) plus
either `review-pack-rule` (pack-origin rules) or `criteria` (repo-derived
rules) evidence, and carries `appliedPolicy`/`originatingPack` — "never
opine without evidence" as a runtime property of every finding this
engine produces, not just a design intention.

## Detector registry (`engine/detectors.ts`)

A `Rule` is declarative data (knowledge-system's own design: "Rules
cannot decide 'this file violates me'; that judgment belongs to a
Skill"). `RULE_DETECTORS` is that judgment: a hand-written, curated
`Record<ruleId, RuleDetector>` — a lightweight regex/line-scan per rule
id, not a generic pattern inferred from the rule's prose. Coverage is
intentionally partial: a resolved `PolicyRule` with no registered
detector is inspectable in the assembled policy but produces zero
findings — CLAUDE.md's "never fakes certainty" applied to detection
coverage.

## Technical decisions

- **Deterministic, not AI-generated, judgment.** The epic asked for a
  reliable review *process*, not prompt engineering, and explicitly ruled
  out new transports. Real per-finding LLM judgment would need MCP
  sampling wired end-to-end (server → host model → structured response)
  — a transport-level capability this package's constraints exclude. A
  curated deterministic detector registry is the defensible v1 within
  those constraints; wiring an actual model call is future work (see
  "Known limitations").
- **`SourceUnit.lineNumbers`** exists specifically so Diff Review can map
  a detector hit in "just the added lines" back to the file's real line
  number — `undefined` means the content already *is* the whole file
  (File/Architecture Review), so line index + 1 is the real line.
- **Layer violation/cycle findings don't need a Review Pack.** They're
  evidenced by the Analysis Engine's own dependency graph, not by an
  authored `Rule` — so they cite `framework-convention` evidence and have
  no `appliedPolicy`/`originatingPack`, unlike every other finding this
  engine produces.

## Known limitations

- **Detector coverage is partial**, by design — currently covers a
  cross-section of the `typescript`/`javascript`/`dart`/`release` packs'
  rules, plus (Epic 16B, Deferred Inventory item G) a handful of
  high-confidence, low-false-positive rules from `react`
  (`no-dangerously-set-inner-html`), `vue` (`use-v-html-carefully`), and
  `rust`/`go` (`no-unwrap-expect`, `no-unsafe-blocks`, the shared
  `no-panic`) — see `RULE_DETECTORS`. This was a deliberately modest
  expansion, not full coverage of any pack, let alone the other ~25
  packs in the Foundation Bundle — closing that fully is out of scope
  for a resolution pass and remains future work (see
  `docs/DEFERRED_INVENTORY.md`). Rules needing real type information or
  structural/AST analysis to avoid unacceptable false-positive rates
  (`no-floating-promises`, `consistent-type-imports`, `no-loop-func`,
  `require-await`, React's `rules-of-hooks`/`exhaustive-deps`, Go's
  `explicit-error-check`) are deliberately left undetected rather than
  approximated with a noisy regex.
- ~~Stack pack rules currently never actually fire end-to-end~~ —
  **fixed as of Epic 12.5.** Rule Resolution now matches
  `requires-language`/`requires-framework` against
  `RepositoryContext.capabilities` (an additive, normalized-id set — see
  `@debuggatha/repository-intelligence`'s README "Repository Capability
  Resolution") instead of `StackProfile.languages`/`.frameworks` exact
  string equality, so the `javascript`/`typescript` stack packs' rules
  now really do resolve and produce findings.
- **Architecture Review does not detect *which* architectural pattern is
  in use** (Clean Architecture, Hexagonal, MVC, MVVM, Feature-First,
  Layered) or adapt its critique to it — the epic's original ambition for
  this Skill. What ships is rule-detector matching across the whole tree
  plus `@debuggatha/analysis-engine`'s layer violations/cycles; pattern
  detection needs real structural/AST analysis this v1 engine doesn't do.
- **No AST/type-aware analysis at all** — every detector is a line-level
  regex scan. This is fast and dependency-free but cannot see across
  multiple lines/statements, so some rules are unimplementable without
  producing unacceptable false positives (see above).

## Semantic review (Epic 13)

`runSemanticFindings` (`engine/semantic-findings.ts`) is this package's
side of the Runtime Engine integration
(`@debuggatha/runtime-engine`) — the "real per-finding LLM judgment"
this README's "Technical decisions" section above named as future work
now has a path, without any Review Skill knowing which provider serves
it. It converts a `RuntimeEngine`'s `SemanticFindingCandidate[]` into
real `Finding[]` via `createFinding`, citing `code` evidence straight
from the candidate (never fabricated) and defaulting severity to
`"medium"` when the model gave no `severityHint`.

Deliberately **not** wired into `reviewFiles`/`reviewDiff`/
`reviewArchitecture` — those three stay synchronous, matching every
existing adapter (`@debuggatha/core`'s `executeReview`, MCP, CLI, VS
Code) that calls them synchronously today. `runSemanticFindings` is an
async, opt-in function a caller composes explicitly:

```ts
const deterministic = reviewFiles(paths, policy);
const semantic = await runSemanticFindings(runtime, units, policy);
const findings = [...deterministic, ...semantic];
```

See `@debuggatha/runtime-engine`'s own README for the full Runtime
Engine architecture, provider abstraction, streaming/cancellation, and
context budgeting this function delegates to.

## Remaining work

The epic's Analysis Skills tier (Dependency Graph, Ownership Detection,
Change Impact) shipped separately as `@debuggatha/analysis-engine`
(Epic 12) and is consumed here via `buildAnalysisFindings`; see that
package's own README for its architecture and limitations. Still open:
decide whether/how to wire real MCP sampling for judgment-requiring
rules this engine's detector registry can't safely approximate.
