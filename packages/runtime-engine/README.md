# @debuggatha/runtime-engine

Epic 13. A provider-agnostic execution layer for semantic (model-powered)
review — the counterpart to Epic 11's deterministic engine. See
[CLAUDE.md](../../CLAUDE.md) at the repo root.

## Architecture overview

```text
Review Skills (@debuggatha/skills)
        │  runSemanticFindings(runtime, units, policy)
        ▼
RuntimeEngine
        │  resolveRuntime(selection) → { provider, model }   (Runtime Selection)
        │  buildSemanticPrompt(units, budget, policyStatements) (Context Budgeting)
        ▼
Provider (Ollama | LM Studio | ...)
        │  execute({ model, prompt, signal }) → AsyncIterable<SemanticStreamEvent>
        ▼
Language Model
        │  raw text response
        ▼
parseSemanticResponse(rawText) → SemanticReviewResult { candidates, parseWarnings }
```

`SemanticReviewResult` — never raw text — is the contract the rest of
Debuggatha ever sees. A provider's `execute()` streams `"token"` events
(raw text as it arrives); `consumeToCompletion` accumulates them and
parses the whole thing once streaming ends, or a provider that itself
understands structured output can emit a `"result"` event directly and
skip parsing. Either way, everything past this boundary — Review Skills,
`@debuggatha/core`, eventually a real `Finding` — only ever handles
`SemanticFindingCandidate[]`, never a provider's own text format.
Swapping Ollama for LM Studio (or any future provider) changes nothing
past this boundary.

## Provider abstraction

`Provider` (`provider.ts`) exposes exactly five capabilities, never a
provider's own wire format: `discoverModels`, `healthCheck`, `execute`
(streaming, cancellable via `AbortSignal`), and `contextWindowFor`. A
`ProviderRegistry` (`registry.ts`) is injectable, in-memory (same
discipline as every prior epic's registry/cache) — adding a new provider
means writing one `Provider` implementation and registering it; nothing
in `engine.ts` changes.

## Implemented providers

- **Ollama** (`providers/ollama.ts`) — `/api/tags` (model discovery +
  health), `/api/generate` with `stream: true` (NDJSON, one JSON object
  per line — `providers/http-stream.ts#readNdjsonLines`), `/api/show`
  for context window (`model_info.*.context_length`).
- **LM Studio** (`providers/lmstudio.ts`) — OpenAI-compatible:
  `/v1/models` (discovery + health, including `max_context_length` when
  reported), `/v1/chat/completions` with `stream: true` (SSE —
  `providers/http-stream.ts#readServerSentEvents`).

Both accept an injectable `fetchImpl` (defaults to global `fetch`) for
testing without a real server, and both share `http-stream.ts`'s chunk
readers — a future OpenAI-compatible provider (Epic 13 "Examples") can
reuse `readServerSentEvents` directly; a future NDJSON-streaming provider
can reuse `readNdjsonLines`.

## Runtime selection

Three modes (`RuntimeSelection` in `engine.ts`), matching the epic
exactly:

- `{ mode: "automatic" }` — first provider whose `healthCheck()` reports
  available, then that provider's first discovered model.
- `{ mode: "provider", providerId, model? }` — a named provider, an
  explicit model or its first discovered one.
- `{ mode: "model", model }` — whichever registered provider's
  `discoverModels()` includes that model id.

All three throw a typed `RuntimeSelectionError` naming exactly what
wasn't available, rather than silently falling back — a Review Skill
calling `runSemanticFindings` sees a clear failure, never a wrong model.

## Streaming

`Provider.execute()` returns an `AsyncIterable<SemanticStreamEvent>`
(`"token" | "result" | "error" | "done"`) — the engine's
`streamSemanticReview()` exposes this directly to a caller that wants to
render partial output; `runSemanticReview()` is the convenience that
drains it via `consumeToCompletion` for a caller that only wants the
final `SemanticReviewResult`.

## Cancellation

Every execution owns an internal `AbortController`
(`engine.ts#ownedController`). An external `AbortSignal` passed into
`runSemanticReview`/`streamSemanticReview` is bridged into it; the
`cancel()` returned from `streamSemanticReview` aborts the same
controller directly. Either path aborts the actual `fetch` call inside
the active provider — "avoid orphaned requests" is satisfied because
there is exactly one controller per execution, and both entry points
(external signal, explicit `cancel()`) drive the same one.

## Context budgeting strategy

- **Token estimation** (`context-budget.ts#estimateTokens`): ~4
  characters per token — a standard, deliberately conservative heuristic
  (over- rather than under-estimates), since most local providers don't
  expose a real tokenizer over HTTP.
- **Budgeting** (`budgetSourceUnits`): greedily includes source units
  until the (context window − reserved tokens) budget is exhausted.
  Every excluded unit is named with a reason — never silently dropped.
- **Chunking** (`chunkSourceUnit`): before budgeting, any unit larger
  than half the available budget is split into line-ranged sub-units
  (`file:start-end`), so a single large file contributes as much as fits
  instead of being excluded wholesale.
- **Repository chunk selection** is the caller's responsibility (Review
  Skills already reduce repository context before this package ever sees
  it — "the deterministic stage should reduce the amount of repository
  context sent to the model," Epic 13 "Integration"): this package
  budgets/chunks whatever `SemanticSourceUnit[]` it's handed, it doesn't
  decide which files matter.

## Integration with Review Skills

`@debuggatha/skills`' `runSemanticFindings` (`engine/semantic-findings.ts`)
is the only place in that package that knows what a `RuntimeEngine` is —
no Review Skill imports a `Provider` or a specific provider factory
directly. It's a **separate, opt-in function**, not a change to
`reviewFiles`/`reviewDiff`/`reviewArchitecture`'s signatures: those three
are synchronous today and every adapter (`@debuggatha/core`'s
`executeReview`, MCP, CLI, VS Code) calls them synchronously. Making
semantic execution mandatory would require making all three — and every
caller — async, a much larger change than this epic's scope. A caller
that wants semantic findings composes them explicitly:

```ts
const deterministic = reviewFiles(paths, policy);
const semantic = await runSemanticFindings(runtime, units, policy);
const findings = [...deterministic, ...semantic];
```

— "Deterministic analysis → Semantic review → Final findings" (Epic 13),
composed by the caller. See `packages/skills/README.md`'s Epic 13 note
for the same reasoning from that package's side.

## Runtime metadata

`RuntimeExecutionMetadata` (returned alongside `runSemanticReview`'s
result, never folded into it): `provider`, `model`, `contextWindow`,
`durationMs`, `estimatedPromptTokens` — enough for a caller to log or
display "this review ran against Ollama/llama3, ~1200 estimated prompt
tokens, 2.1s" without this package exposing anything about how that
provider actually works.

## Testing

49 tests: provider abstraction/registry, runtime selection (all three
modes, success and failure), streaming, cancellation (both the
`cancel()` path and an externally-bridged `AbortSignal`), context
budgeting (inclusion/exclusion/chunking), response parsing (well-formed,
fenced, malformed, missing-evidence rejection), and both providers
(model discovery, health, streaming, mid-stream provider errors,
context-window resolution) via an injected `fetchImpl` — no real network
calls, no real Ollama/LM Studio instance required to run the suite.

## Known limitations / future provider extension points

- **No OpenAI/Anthropic/Gemini-compatible provider ships yet** — the
  epic names them as extension points, not requirements for this pass.
  `providers/http-stream.ts#readServerSentEvents` is already
  OpenAI-wire-format-shaped, so an OpenAI-compatible provider is mostly
  a new `providers/openai.ts` reusing it, not new infrastructure.
- **No cloud authentication** (explicitly out of scope for this epic) —
  every provider here is a local, unauthenticated HTTP endpoint.
- **Repository chunk *selection*** (deciding *which* files matter most
  for a semantic pass) is not this package's job and isn't implemented
  here — see "Context budgeting strategy" above. `@debuggatha/skills`
  (or a future Review Skill) is expected to hand this package an
  already-reduced `SemanticSourceUnit[]`.
- **Token estimation is a heuristic**, not a real tokenizer — accurate
  enough to budget conservatively, not to report exact token counts.
