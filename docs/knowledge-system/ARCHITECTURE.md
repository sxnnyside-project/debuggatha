# Knowledge System — Architecture (Epic 3)

Status: approved (with one amendment — see ADR-0002), not yet
implemented. Implementation is Epic 3's second phase; this document plus
`docs/adr/0001`–`0005` is its source of truth. See [CLAUDE.md](../../CLAUDE.md)
for product context, the
[Repository Intelligence README](../../packages/repository-intelligence/README.md)
(Epic 1, implemented) and the [Review Engine README](../../packages/review-engine/README.md)
(Epic 2, implemented) for the two subsystems this proposal builds on top
of without modifying.

## 1. Why this document exists

Epic 1 produces `RepositoryContext` — what a repository *is*. Epic 2
defines what a review *produces* (`Finding`, `ReviewResult`,
`ReviewSession`) but deliberately knows nothing about Review Packs or
Review Policies — it only carries `ReviewPackReference { id }` and
`ReviewPolicyReference { id }` as forward references. Nothing in the
codebase today turns those `id` strings into real content, and nothing
turns a Review Pack's static Rules plus a repository's resolved Criteria
into an actual `ReviewPolicy` a Skill can execute against.

The Knowledge System is Epic 3: it defines what a Skill, a Review Pack,
and a Review Policy *are*, how they're discovered, how they combine, and
how a `ReviewPackReference{id}` / `ReviewPolicyReference{id}` becomes real
content that a Review Skill (`reviewArchitecture`, `reviewDiff`, and
whatever follows) can execute and a `Finding` can cite.

## 2. Research: patterns considered and their relevance

The tools below solve overlapping problems — how to represent, discover,
scope, compose, and version pluggable capability. Each pattern was
evaluated for **why it applies (or explicitly doesn't)** to Debuggatha,
not copied wholesale.

| System | Core pattern | Relevance to Debuggatha |
|---|---|---|
| **Claude Code Agent Skills** | Progressive disclosure: lightweight YAML frontmatter (id, description) is always loaded into context; full instruction body loads only when the skill is actually invoked. Skills are discovered from the filesystem at session start; invocation is model-decided, driven by the description field. | Directly relevant to **Capability Registry** and **Skill discovery**: keep a cheap, always-available index of *what exists* (id, tier, description) separate from the (potentially large) logic each Skill runs. Not relevant: filesystem-scanned, model-invoked discovery — Debuggatha's Skills are compiled TypeScript, not natural-language-triggered documents, and Core Skills must run deterministically regardless of model judgment (CLAUDE.md's non-negotiable). We borrow the *shape* (cheap descriptor vs. expensive body), not the *mechanism* (filesystem + LLM-driven trigger). |
| **Cursor Rules (`.cursor/rules/*.mdc`)** | Rules are scoped units with declared activation: `alwaysApply`, glob-matched (`Auto Attached`), description-matched (`Agent Requested`), or `Manual`. Nested rule directories scope rules to subdirectories. | Directly relevant to **Rule scoping inside a Review Pack**: a Rule should declare *when it applies* (a glob, a required framework, a required language) rather than firing unconditionally. Not relevant: per-file activation at editor-keystroke granularity — Debuggatha evaluates a whole `ReviewRequest` scope (diff/files/workspace) per session, not per keystroke, so activation is resolved once at Rule Resolution time, not continuously. |
| **Windsurf Rules** | Same activation-mode idea as Cursor, but with hard character budgets per rule file, forcing authors to keep always-on content small and push the rest behind glob/description activation. | Reinforces the same lesson as Cursor from a different angle: unconditional, always-loaded content doesn't scale past a handful of rules. Not relevant: the specific character-limit mechanic — Debuggatha's Rules are typed objects consumed programmatically, not raw text injected into a prompt, so token budget is a Context Assembly concern, not a Rule-authoring constraint. |
| **Language Server Protocol** | Client and server exchange `ClientCapabilities`/`ServerCapabilities` once at `initialize`; unknown properties are ignored, a missing property means "capability absent" (never assumed present); some capabilities register dynamically post-init. | Directly relevant to the **Capability Registry**: a consumer (a Skill, the future MCP layer) should be able to ask "does pack X exist, what does it need, is it available" before depending on it, and an absent capability should be a clean "not available" rather than a crash. Not relevant: live bidirectional RPC negotiation over a wire protocol — Debuggatha's registry is in-process (v1), not a client/server handshake; MCP already owns that layer, and the Knowledge System stays below it. |
| **ESLint (flat config)** | A config is an ordered array of layered objects; each layer can scope itself by file pattern; `extends` composes named rule-sets from plugins; later layers override earlier ones for the same key. Plugins ship rules + rule-sets; configs decide which rules are active and at what severity. | Directly relevant to **Rule Resolution**: the pack-rules-plus-repo-criteria merge in Debuggatha is structurally the same problem as "plugin rules plus user overrides" in ESLint — ordered layering with explicit precedence, not implicit magic. We adopt the *ordered-layers-with-explicit-precedence* idea, not flat config's specific array-of-objects file format (Debuggatha's "config" is programmatically assembled from `RepositoryContext.criteria`, not authored by a human in a config file). |
| **Biome plugin system (GritQL)** | Plugins are declarative pattern-matching content (`.grit` files) registered via a config array; diagnostic-only in its current form — deliberately no autofix yet, shipped narrower than ESLint's plugin surface to keep the contract simple. | Reinforces starting narrower than the most powerful competitor and widening later. Directly informs the decision to keep v1 Rules as declarative, data-shaped citable statements (see §5) rather than executable code — a Rule that can express "you can run arbitrary logic" is a much bigger contract to keep stable and secure than a Rule that says "here is a fact plus a citation." |
| **Terraform providers** | A provider is a separate versioned plugin exposing a *schema* (what resources/attributes exist) over a stable RPC protocol; Terraform Core talks to the schema for planning, and to CRUD operations for execution — schema, state, and execution are three distinct concerns that don't leak into each other. | Directly relevant to the **Pack/Policy/Execution split**: a Review Pack's *schema* (its Rules, what it applies to, what it depends on) must be inspectable without running anything, exactly like `terraform providers schema` doesn't execute any resource. Applying a policy (execution) is a separate step from resolving it (planning). Not relevant: out-of-process plugin binaries over gRPC — Debuggatha's Packs are in-process TypeScript modules in v1; the process-isolation and wire-protocol machinery is unnecessary complexity for a monorepo where every Pack ships in the same release train. |
| **Pulumi Packages** | A language-agnostic *schema* describes resources/functions independent of any SDK; providers are versioned, cached plugin binaries; a resource can pin an exact provider version so old programs keep resolving against the version they were written for. | Directly relevant to **Versioning**: "a resource can pin its provider version so history stays meaningful" is exactly the property a past `ReviewResult` needs with respect to the Review Pack version that produced it (see §8, §9 ADR-0002/0005). Not relevant: bridged-vs-native provider distinction, remote plugin binary downloads — no analogue needed while all Packs ship in-repo. |
| **MCP tools capability model** | Discovery (`tools/list`) is separate from invocation (`tools/call`); `listChanged` notifications let a dynamic tool set update without renegotiating the whole session. | Confirms the Capability Registry should expose a **list/describe vs. invoke** split, and that the registry's content can legitimately change between sessions (a Pack added mid-development) without requiring a new protocol version. Not relevant to the Knowledge System directly — MCP is Debuggatha's distribution layer (CLAUDE.md's non-negotiable: "MCP is distribution only, not the value proposition"), so the Knowledge System defines this list/describe/invoke split at the domain level, and MCP tooling (`packages/mcp`) is just one future client of it, not its origin. |
| **Open Policy Agent (bundles)** | Policy bundles declare a `rego_version`; bundle composition rejects overlapping/conflicting package namespaces at build time rather than silently merging; a policy stack's conflicting decisions get combined by an explicit, documented strategy. | Directly relevant to **conflict handling** (§7, ADR-0003): conflicts should be caught and attributed, never silently merged away. Not relevant: Rego as an executable policy language — Debuggatha's Rules are closer to Biome's GritQL-era decision (declarative, citable, not arbitrary executable logic) for v1. |

**Cross-cutting lesson used throughout this design:** every one of these
systems separates *"what capability exists and what does it need"*
(schema / descriptor / capability) from *"what specific thing did we
compute for this run"* (state / policy / plan) from *"what actually ran"*
(execution / apply / invoke). Debuggatha already has this instinct in
Epic 1 (`RepositoryContext` is a snapshot, not logic) and Epic 2
(`ReviewSession` is process bookkeeping, separate from the `ReviewResult`
deliverable). The Knowledge System extends the same three-way split to
Packs → Policies → applied Findings.

## 3. Domain model

```
                         ┌─────────────────────────┐
                         │   Capability Registry    │
                         │  (what exists, in-proc)  │
                         │  Skills · Packs · deps    │
                         └────────────┬─────────────┘
                                      │ describes / resolves
                                      ▼
 RepositoryContext        ┌───────────────────────┐        ReviewRequest
 (Epic 1, frozen) ───────►│    Rule Resolution     │◄─────── (Epic 2)
  .criteria (CriteriaRule)│  packs' Rules  +        │  requestedPackIds
                          │  repo's CriteriaRule[]  │  requestedPolicyId
                          └────────────┬────────────┘
                                       │ produces
                                       ▼
                         ┌─────────────────────────┐
                         │      ReviewPolicy        │  (resolves what
                         │  id · rules[] · packRefs  │   ReviewPolicyReference
                         │  conflicts[]              │   {id} points at)
                         └────────────┬─────────────┘
                                       │ + RepositoryContext + ReviewRequest
                                       ▼
                         ┌─────────────────────────┐
                         │     Context Assembly      │
                         │   → SkillContext          │
                         └────────────┬─────────────┘
                                       │ executes
                                       ▼
                         ┌─────────────────────────┐
                         │    Skill (Core/Review/    │
                         │    Analysis — code, not   │
                         │    data)                  │
                         └────────────┬─────────────┘
                                       │ produces (Epic 2 types)
                                       ▼
                         ┌─────────────────────────┐
                         │  Finding.evidence includes │
                         │  { kind:"review-pack-rule",│
                         │    packId, ruleId }        │
                         │  Finding.appliedPolicy =    │
                         │    ReviewPolicyReference    │
                         │  Finding.originatingPack =  │
                         │    ReviewPackReference      │
                         └─────────────────────────┘
```

```mermaid
flowchart TD
    RC[RepositoryContext<br/>Epic 1, frozen] -->|criteria: CriteriaRule[]| RR[Rule Resolution]
    CR[Capability Registry] -->|resolved packs + deps| RR
    Req[ReviewRequest<br/>Epic 2] -->|requestedPackIds / requestedPolicyId| RR
    RR --> RP[ReviewPolicy<br/>id, rules, packRefs, conflicts]
    RP --> CA[Context Assembly]
    RC --> CA
    Req --> CA
    CA --> SC[SkillContext]
    SC --> SK[Skill: Core / Review / Analysis]
    SK --> F[Finding<br/>Epic 2: evidence, appliedPolicy, originatingPack]
```

### Where each new type sits relative to existing ones

| Existing type (frozen, do not modify) | New Knowledge System type | Relationship |
|---|---|---|
| `RepositoryContext.criteria: CriteriaProfile` (`CriteriaRule[]`, repository-intelligence) | `ReviewPolicy.rules[]` (knowledge-system) | Rule Resolution merges `CriteriaRule[]` (repo's own resolved criteria) with a Pack's `Rule[]` into one `ReviewPolicy.rules[]`, tagging each with provenance. |
| `ReviewPackReference { id: string; version: string }` (review-engine) | `ReviewPack { id, version, kind, rules, knowledge, dependsOn, ... }` (knowledge-system) | The reference is the pointer; the Pack is the content the Capability Registry resolves that pointer to. `version` is a real, structured field on the reference — see ADR-0002 (revised after product review to add this field to review-engine rather than overload `id` with a string convention). |
| `ReviewPolicyReference { id: string }` (review-engine) | `ReviewPolicy { id, rules, packRefs, conflicts, ... }` (knowledge-system) | Same relationship. `id` is a deterministic hash of its inputs — see ADR-0005. |
| `Evidence` discriminated union, specifically `{ kind: "review-pack-rule"; packId; ruleId }` and `{ kind: "criteria"; ruleId; source }` (review-engine) | `Rule.id` / `CriteriaRule.id` | Already-shipped evidence variants are exactly the join keys a `Finding` needs to cite a Knowledge System `Rule` or a repo `CriteriaRule` — no new Evidence variant is needed. |
| `ReviewSession.selectedPolicy` / `selectedPacks` (review-engine) | Output of Rule Resolution | Whatever produces a session (a future `packages/core` orchestrator) calls Rule Resolution, gets back a `ReviewPolicy`, and stores its reference on the session — the Knowledge System never constructs a `ReviewSession` itself. |
| `ReviewSource` (`ai-provider` \| `deterministic-analyzer` \| `custom-skill` \| `human-review`) | `SkillDescriptor.tier` (`core` \| `review` \| `analysis`) | Orthogonal axes: `ReviewSource` says *what kind of thing* produced a Finding; `SkillDescriptor.tier` says *which taxonomy tier* the Skill belongs to (CLAUDE.md's Core/Review/Analysis). A Review Skill could be backed by any `ReviewSource`. |

## 4. Main components and responsibilities

**Capability Registry** (`@debuggatha/knowledge-system`)
Answers "what exists and what does it need" — nothing else. Holds
`SkillDescriptor[]` and `ReviewPack[]` (metadata, not necessarily full
rule bodies loaded eagerly — see §11 on YAGNI for lazy loading). Resolves
a Pack's declared dependencies against what's registered and reports
missing ones. Never executes anything, never sees a `RepositoryContext`.
Injectable, not a global singleton — same discipline as
`RepositoryContextCache` in Epic 1: a caller constructs an instance and
controls its lifetime; a static, pre-populated default instance ships for
v1's fixed pack catalog (see ADR-0004).

**Rule Resolution** (`@debuggatha/knowledge-system`)
A pure function: `(RepositoryContext, requested pack ids/policy id,
CapabilityRegistry) → ReviewPolicy`. Filters each Pack's Rules by their
`appliesTo` scoping against `RepositoryContext.stack`, merges the
survivors with `RepositoryContext.criteria.rules`, detects conflicts
(§7), and returns a frozen `ReviewPolicy` — mirroring `RepositoryContext`'s
own `Object.freeze` discipline. Throws (not silently degrades) if a
requested Pack's dependency can't be resolved (ADR-0003 territory).

**Context Assembly** (`@debuggatha/knowledge-system`, consumed by whatever
orchestrates a session — currently `@debuggatha/core`'s future job)
A pure function: `(RepositoryContext, ReviewRequest, ReviewPolicy |
undefined) → SkillContext`. `SkillContext` is the one shape every Skill
receives, regardless of `ReviewSource` — Core Skills receive it with
`policy: undefined` (they don't need one; they run before any Pack is
selected, per CLAUDE.md "never let a Review Skill run before Core Skills
have oriented on the repo"), Review/Analysis Skills receive it with a
resolved `policy`.

**Review Packs** (`@debuggatha/review-packs`)
Content, not logic — a library of `ReviewPack` values (stack packs:
React/TS, Kotlin...; concern packs: OWASP, DX...) conforming to the
schema `@debuggatha/knowledge-system` defines. This package should
contain **no algorithms** — no resolution, no conflict handling, no
registry logic — only data plus the validation call that proves each
Pack is well-formed at build/test time. This mirrors Terraform providers
and ESLint plugins: the plugin author ships a schema-conformant bundle;
the engine that interprets it lives elsewhere.

**Policies** (`@debuggatha/policies`)
The thin orchestration package that wires a `CapabilityRegistry`
instance (pre-populated with `@debuggatha/review-packs`' content) to
`@debuggatha/knowledge-system`'s `resolvePolicy` function, and exposes
`assemblePolicy(...)` as the stable call site the rest of the system
uses. It is intentionally almost empty — the algorithm lives in
`knowledge-system` so it can be unit-tested without importing real pack
content, and so `knowledge-system` can be imported by `packages/skills`
directly without a dependency on the concrete pack catalog.

**Skills** (`@debuggatha/skills`)
Executes. A Skill is a function `(SkillContext) → Finding[] |
AnalysisResult` (Analysis Skills return structural facts, not Findings —
CLAUDE.md: "discover, don't judge"). This is where `reviewArchitecture`
and `reviewDiff` stop being throw-stubs and stop importing
`@debuggatha/shared`'s legacy types — see ADR-0001 and the migration this
implies.

## 5. Rules and Knowledge — the atomic content shape

```ts
interface Rule {
  id: string;                    // stable within its pack, e.g. "no-any-in-public-api"
  packId: string;                // the pack that owns it (redundant with ReviewPack.rules
                                  // containment, kept for cheap citation without pack lookup)
  statement: string;              // one atomic, individually citable sentence
  category: Category;             // review-engine's open Category — reused, not reinvented
  appliesTo: RuleScope;           // when this rule is even relevant (see below)
  defaultSeverity: Severity;      // a starting point; a Skill may adjust per-instance,
                                   // never the other way (Severity/Confidence independence
                                   // from review-engine — do not derive one from the other)
  knowledgeRefs: string[];        // ids into this pack's KnowledgeEntry[] — every Rule
                                   // must cite at least one, same "no source, no finding"
                                   // discipline CLAUDE.md already applies to Findings
}

type RuleScope =
  | { kind: "always" }
  | { kind: "requires-framework"; framework: string }
  | { kind: "requires-language"; language: string }
  | { kind: "glob"; pattern: string };

interface KnowledgeEntry {
  id: string;
  title: string;
  body: string;                   // the underlying domain expertise a Rule derives from
  externalRefs: string[] | undefined; // e.g. an OWASP/WCAG URL, for concern packs
}
```

Rules are **declarative data**, not executable code (the Biome-GritQL
lesson from §2: start narrower). A Rule cannot itself decide "this file
violates me" — that judgment is the Skill's job, using the Rule as a
citable constraint. This keeps Packs safe to load from anywhere
(including, later, a third party) without a code-execution trust
boundary, and keeps the domain testable without mocking an execution
sandbox.

## 6. Lifecycle: defined → applied

1. **Defined** — a `ReviewPack` value is authored in
   `@debuggatha/review-packs` (or, later, an external package — see §10)
   and validated against the `knowledge-system` schema (`validateReviewPack`,
   a pure function: checks every `Rule.knowledgeRefs` resolves, every
   `dependsOn` entry is well-formed, `id`/`version` are present).
2. **Registered** — the pack is added to a `CapabilityRegistry` instance
   (v1: a static array, mirroring `packages/review-packs/src/index.ts`'s
   already-scaffolded `reviewPacks: readonly []`, populated for real).
3. **Requested** — a `ReviewRequest.requestedPackIds` (or
   `requestedPolicyId`, for re-running a previously assembled policy)
   names which Packs/Policy a session wants — this field already exists
   in review-engine untouched.
4. **Resolved** — Rule Resolution turns the request plus the session's
   `RepositoryContext` into a concrete, frozen `ReviewPolicy`, checking
   dependencies via the Capability Registry and recording any conflicts.
5. **Assembled** — Context Assembly combines that `ReviewPolicy` with the
   `RepositoryContext` and `ReviewRequest` into a `SkillContext`.
6. **Applied** — a Skill executes with that `SkillContext` and produces
   `Finding[]`, each citing the specific `Rule` (via the existing
   `review-pack-rule` Evidence variant) or `CriteriaRule` (via the
   existing `criteria` Evidence variant) that justified it, and stamping
   `appliedPolicy` / `originatingPack` — both fields already exist on
   `Finding` and are currently always `undefined` in practice because
   nothing populates them yet.

Every step from 4 onward is pure and produces an immutable value — no
step in this pipeline mutates a `RepositoryContext`, a `ReviewPack`, or a
previously produced `ReviewPolicy`.

## 7. Knowledge composition and conflict handling

**Composition (the non-conflicting case).** A session may select several
Packs (e.g. a stack pack + a concern pack). Rule Resolution's default
behavior is straightforward union: collect every Rule from every
requested Pack whose `appliesTo` matches the `RepositoryContext.stack`,
plus every `CriteriaRule` from `RepositoryContext.criteria`. No pack
"owns" a review; policies are additive by default, same as ESLint's
config-array layering or an OPA bundle with non-overlapping packages.

**Conflict definition (v1, deliberately narrow).** Two rules conflict
when they share a `concernKey` — v1 defines this as an exact match on
`(category, appliesTo)` — but make opposite claims (this requires each
Rule's `statement` to be tagged, at authoring time, with a `contradicts:
string[]` list of rule ids it's known to conflict with; nothing attempts
automatic semantic contradiction detection — that's a much harder,
speculative NLP problem, explicitly deferred, see §11). Two packs that
happen to both have opinions about "line length," say, are not
automatically flagged unless one pack's author explicitly declared the
conflict.

**Resolution policy.** When a conflict is detected: the repository's own
`CriteriaRule` always wins over any Pack's `Rule` (the repo's stated
intent is ground truth — same evidence hierarchy CLAUDE.md already states
for Findings: "a rule from the repository itself" is listed before "a
Review Pack"). Between two Packs, the one explicitly named earlier in
`ReviewRequest.requestedPackIds` wins. **The losing rule is never
silently dropped** — `ReviewPolicy.conflicts: ConflictRecord[]` records
both rule ids, which won, and why, so a `ReviewResult` can be inspected
later to see that a conflict existed even though only one rule was
applied. This is ADR-0003.

## 8. Versioning strategy

Packs are semver-versioned (`ReviewPack.version`). A pack reference is
carried as a real, structured field — `ReviewPackReference { id: string;
version: string }` — added to `@debuggatha/review-engine` as a small,
additive, non-breaking change (see ADR-0002; revised after product review
from the original proposal of overloading `id` with a `"packId@version"`
string). `ReviewRequest.requestedPackIds: string[]` (already shipped in
Epic 2, unchanged) stays bare pack ids — a request names *which packs*,
not *which version*; the Capability Registry resolves the concrete
version at Rule Resolution time.

A `ReviewPolicy.id` is a deterministic hash over its exact inputs (sorted
`{id, version}` pairs actually applied + the `RepositoryContext`'s
`generatedAt`/fingerprint + a `KNOWLEDGE_SCHEMA_VERSION` constant). Two
runs with the same repository state and the same pack versions produce
the same policy id — reproducibility over cleverness, the same lesson
Terraform's plan determinism and Pulumi's per-resource version pinning
both encode. This is ADR-0005.

**What happens when a pack changes after a `ReviewResult` already cited
it:** the old `ReviewResult.findings[].appliedPolicy` / `.originatingPack`
still point at the exact `packId@version` that produced them — a pack
publishing `2.2.0` does not retroactively change what `"react-ts@2.1.0"`
means, *provided published pack versions are treated as immutable* (never
edited in place — a content or behavior change requires a new version).
This package-level discipline is a convention this document recommends,
not a runtime-enforced invariant in v1 (there is no package registry to
enforce it against yet — see §11). Whether the exact rule *content* of
`"react-ts@2.1.0"` remains fetchable after `2.2.0` ships is a
**Findings ledger** concern (CLAUDE.md: "designed, not yet built") — the
Knowledge System guarantees the reference is stable and unambiguous; it
does not itself guarantee eternal storage of old pack content, which is
out of scope for an in-repo monorepo with no artifact registry yet.

## 9. Dependency relationships

`ReviewPack.dependsOn: PackDependency[]` where `PackDependency = { packId:
string; versionRange: string } | { skillId: string }`. Example: a future
Tauri pack depends on the Rust pack (`{ packId: "rust", versionRange:
"^1.0.0" }`) and on a `Change Impact` Analysis Skill being registered
(`{ skillId: "change-impact" }`).

**Missing dependency handling is fail-closed, not degrade-gracefully.**
If a requested pack's dependency isn't in the Capability Registry, Rule
Resolution throws — it does not silently assemble a partial policy. This
matches the identity principle "never fakes certainty": a policy silently
missing half its intended rules because a dependency was absent is a
worse failure mode than a clear, early error naming exactly which
dependency was missing and which pack needed it. No soft/optional
dependencies exist in v1 — see §11.

## 10. Extension model — adding a new Review Pack

1. Author a `ReviewPack` value (id, version, kind, rules, knowledge,
   dependsOn) in `packages/review-packs/src/packs/<name>.ts`, following
   the shape in §5.
2. Run `validateReviewPack` (a pure function this epic adds to
   `knowledge-system`) — checks every rule cites real knowledge, every
   dependency is well-formed, ids are unique within the pack. This should
   run in the pack's own test suite, the same discipline
   repository-intelligence already applies to its scanners.
3. Add the pack to the exported registry array in
   `packages/review-packs/src/index.ts` (today: `export const reviewPacks:
   readonly [] = []`; this epic's job is making that array real).
4. No change is required in `review-engine`, `policies`, `skills`, `mcp`,
   or `cli` — a well-formed Pack is immediately requestable via
   `ReviewRequest.requestedPackIds` because those packages depend on the
   `knowledge-system` schema, not on any specific pack's content. This is
   the entire point of the schema/content split borrowed from Terraform
   providers and ESLint plugins (§2).
5. **Out of scope for this extension path (v1):** publishing a pack as an
   installable external artifact, discovering packs from the filesystem
   or npm at runtime, or a marketplace/registry service. Every v1 pack
   ships in this monorepo's release train — see §11.

## 11. Explicitly YAGNI for the near term

- **Filesystem or network pack discovery.** Claude Code Skills, Cursor
  Rules, and MCP servers all support out-of-tree discovery; Debuggatha's
  v1 targets 2–3 packs shipped in-repo (CLAUDE.md v1 scope). A static,
  compiled-in registry array is sufficient and testable; building a
  loader/sandbox for third-party pack code before a third party exists to
  author one is speculative complexity this document explicitly declines
  to add.
- **Executable/scriptable Rules.** Keeping Rules as declarative data
  (§5) avoids a code-execution trust boundary. Revisit only if a real
  Pack author hits a limit static rules can't express.
- **Automatic semantic conflict detection between rules.** v1 only
  catches conflicts a pack author explicitly declares via `contradicts`.
  NLP-based contradiction detection between two prose `statement` fields
  is a real capability some day, not a v1 requirement.
- **Soft/optional pack dependencies.** Every dependency is hard; a
  "use if available, otherwise skip" mode adds a second failure-handling
  path this document has no evidence is needed yet.
- **Cross-process/plugin-isolation for Packs** (Terraform/Pulumi's
  separate-binary-over-RPC model). Unnecessary while every Pack ships in
  the same monorepo release; would only become relevant if Debuggatha
  ever needs to sandbox untrusted third-party pack code.
- **A persisted Policy Store / pack version archive.** `ReviewPolicy.id`
  is deterministic and reproducible *given the same pack versions are
  still available*, but this epic does not build storage that guarantees
  old pack versions stay available forever — that's the Findings ledger's
  job (already deferred in CLAUDE.md) or a future package registry's job,
  not the Knowledge System's.

## 12. Risks

- **Rule Resolution's precision is bounded by Criteria Resolution's
  precision.** `CriteriaRule` extraction from Markdown is heading-level,
  not semantic (repository-intelligence's own documented risk). Any
  `ReviewPolicy` built partly from `RepositoryContext.criteria` inherits
  that imprecision — a "rule" resolved from a repo's own docs may be a
  section title, not a verified constraint. Skills consuming
  `ReviewPolicy.rules` must treat repo-derived entries with the same
  caution repository-intelligence's README already recommends for
  `CriteriaProfile` consumers directly.
- **`ReviewPackReference` gained a required `version` field (ADR-0002,
  revised after product review), a small breaking change to Epic 2's
  otherwise-frozen review-engine package.** Every existing construction
  site needed updating (mechanical, already done). Low risk today because
  review-engine has no external consumers yet outside this monorepo; would
  be a real breaking-change concern once something depends on it from
  outside the Debuggatha release train.
- **Fail-closed dependency handling (§9) has no escape hatch in v1.** If
  a widely-depended-on pack becomes unavailable (e.g. removed from the
  registry mid-session), every pack depending on it stops resolving
  entirely. Acceptable for a 2–3 pack v1 catalog; worth revisiting once
  the catalog grows (CLAUDE.md's deferred "full Review Pack catalog").
- **No enforcement that published pack versions stay immutable.** §8's
  "never edit a published version in place" is a convention, not a
  runtime check — nothing in this design stops someone from editing
  `packages/review-packs/src/packs/react-ts.ts` without bumping
  `version`. A lint/CI check (diff the pack's rules against its declared
  version, fail if they changed without a version bump) is a reasonable
  future addition but is not proposed as part of this epic.

## 13. Summary of new packages/types vs. what's untouched

- **New:** `@debuggatha/knowledge-system` — the domain package for
  `SkillDescriptor`, `ReviewPack`, `Rule`, `KnowledgeEntry`,
  `CapabilityRegistry`, `resolvePolicy` (Rule Resolution), `assembleContext`
  (Context Assembly), `ReviewPolicy`, `ConflictRecord`,
  `validateReviewPack`. Depends on `@debuggatha/repository-intelligence`
  and `@debuggatha/review-engine` (for `RepositoryContext`, `Evidence`,
  `ReviewPackReference`, `ReviewPolicyReference`, `Category`, `Severity`)
  the same direction Epic 2 already depends on Epic 1 — never the
  reverse.
- **Filled in, not restructured:** `@debuggatha/review-packs` (content),
  `@debuggatha/policies` (thin orchestration wiring registry +
  `resolvePolicy`), `@debuggatha/skills` (Skill implementations migrate
  off `@debuggatha/shared`'s legacy `Finding`/`ReviewPolicy` onto
  `@debuggatha/review-engine` + `@debuggatha/knowledge-system` types —
  this is the migration review-engine's README already flagged as owed).
- **Untouched:** `@debuggatha/repository-intelligence` (Epic 1),
  `@debuggatha/review-engine` (Epic 2) — every type this document
  references from those packages (`RepositoryContext`, `CriteriaRule`,
  `Evidence`, `ReviewPackReference`, `ReviewPolicyReference`, `Finding`,
  `ReviewSession`, `Category`, `Severity`, `Confidence`) is consumed
  as-is, with no proposed field additions or breaking changes.
