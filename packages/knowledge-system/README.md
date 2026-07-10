# @debuggatha/knowledge-system

Epic 3. Defines what a Skill, a Review Pack, and a Review Policy *are*, how
they're discovered, how they combine, and how a
`ReviewPackReference{id}` / `ReviewPolicyReference{id}` (review-engine
stand-ins) become real content a Review Skill can execute against and a
`Finding` can cite. See [CLAUDE.md](../../CLAUDE.md) at the repo root and
[`docs/knowledge-system/ARCHITECTURE.md`](../../docs/knowledge-system/ARCHITECTURE.md)
plus `docs/adr/0001`–`0005` for the product decisions this package encodes.

This package does **not** talk to an AI provider, does not know what MCP,
the CLI, or VS Code are, and ships no real Review Pack content itself — it
depends on `@debuggatha/repository-intelligence` (Epic 1, for
`RepositoryContext`/`CriteriaRule`) and `@debuggatha/review-engine`
(Epic 2, for `Evidence`/`Category`/`Severity`/`ReviewPackReference`/
`ReviewPolicyReference`), the same dependency direction Epic 2 already
established relative to Epic 1 — never the reverse.

## Architecture summary

```text
CapabilityRegistry (Skills + Packs, injectable, in-memory)
        │ resolveDependencies(requestedPackIds)
        ▼
resolvePolicy(RepositoryContext, ReviewRequest, CapabilityRegistry)
        │  1. resolve requested packs + transitive dependencies (fail-closed)
        │  2. filter each pack's Rules by appliesTo vs. RepositoryContext.stack
        │  3. union survivors with RepositoryContext.criteria.rules
        │  4. detect + resolve conflicts (ADR-0003), record ConflictRecord[]
        │  5. hash the effective inputs into a deterministic id (ADR-0005)
        ▼
ReviewPolicy { id, rules, packRefs, conflicts }  ── frozen
        │
        ▼
assembleContext(RepositoryContext, ReviewRequest, ReviewPolicy | undefined)
        ▼
SkillContext { repository, request, policy }  ── the one shape every Skill receives
```

`validateReviewPack` is a separate, pure structural check run over a
`ReviewPack` value at authoring/test time (Architecture doc §6 step 1) —
it does not participate in the `resolvePolicy` pipeline itself.

## Domain model overview

| Type | Role |
|---|---|
| `SkillDescriptor` (`id`, `tier`, `description`) | The cheap, always-loadable index entry for a Skill — CLAUDE.md's Core/Review/Analysis taxonomy as a `tier`. |
| `Rule` | Atomic, individually citable, declarative (not executable) statement owned by a pack — `appliesTo` scopes it, `knowledgeRefs` cites at least one `KnowledgeEntry`, `contradicts` opts in to conflict detection (ADR-0003). |
| `RuleScope` | `always` \| `requires-framework` \| `requires-language` \| `glob` — when a Rule is even relevant. |
| `KnowledgeEntry` | The underlying domain expertise a Rule derives from. |
| `ReviewPack` | Content, not logic: `id`, `version` (semver), `kind` (`stack` \| `concern`), `rules`, `knowledge`, `dependsOn`. |
| `PackDependency` | A hard dependency on another pack (`{ packId, versionRange }`) or a Skill (`{ skillId }`) — no soft/optional dependencies in v1. |
| `CapabilityRegistry` | Interface: `listSkills`, `listPacks`, `getPack`, `resolveDependencies` — answers "what exists and what does it need," never executes anything (ADR-0004). `createInMemoryRegistry` is the v1 implementation. |
| `PolicyRule` | The unified shape `ReviewPolicy.rules` carries — a Pack `Rule` or a repo `CriteriaRule`, tagged with `origin` provenance. |
| `ConflictRecord` | `{ winningRuleId, losingRuleId, reason }` — the losing rule is never dropped from the record, only from `ReviewPolicy.rules` (ADR-0003). |
| `ReviewPolicy` | `{ id, rules, packRefs, conflicts }` — what `ReviewPolicyReference{id}` (review-engine) resolves to; `id` is a deterministic hash (ADR-0005). |
| `SkillContext` | `{ repository, request, policy }` — the one shape every Skill receives; Core Skills get `policy: undefined`. |
| `resolvePolicy` | Pure: `(RepositoryContext, ReviewRequest, CapabilityRegistry) → ReviewPolicy`. Fail-closed on missing dependencies. |
| `assembleContext` | Pure: `(RepositoryContext, ReviewRequest, ReviewPolicy \| undefined) → SkillContext`. |
| `validateReviewPack` | Pure: throws a single aggregated `Error` naming every structural issue in a `ReviewPack`. |

## Implemented capabilities

- **Skill Registry / abstractions** — `SkillDescriptor`, `SkillTier`
  (`core` \| `review` \| `analysis`).
- **Review Pack Registry** — `CapabilityRegistry` interface +
  `createInMemoryRegistry`, injectable (ADR-0004): lists Skills/Packs,
  resolves a pack by id (optionally against a version range), and walks
  `dependsOn` transitively, reporting exactly which dependency (pack or
  Skill) is missing rather than silently dropping it.
- **Review Pack loading & validation** — `ReviewPack`/`Rule`/`RuleScope`/
  `KnowledgeEntry`/`PackDependency` types (Architecture doc §5, §9) and
  `validateReviewPack`: unique rule/knowledge ids within a pack, every
  `Rule.knowledgeRefs` resolves to a real `KnowledgeEntry`, every
  `dependsOn` entry is well-formed (exactly one of `packId`+`versionRange`
  or `skillId`), `id`/`version` present and `version` semver-shaped.
- **Review Policy domain** — `ReviewPolicy`, `PolicyRule`,
  `PolicyRuleOrigin`, `ConflictRecord` (Architecture doc §7).
- **Rule Resolution** — `resolvePolicy`: unions applicable pack rules
  (filtered by `appliesTo` against `RepositoryContext.stack`) with the
  repository's own `CriteriaRule[]`, detects conflicts, applies ADR-0003's
  fixed precedence (repo criteria > earlier-requested pack), and returns
  a frozen `ReviewPolicy`.
- **Context Assembly** — `assembleContext`.
- **Versioning** — `ReviewPolicy.id` is a truncated SHA-256 over a
  canonical (sorted-keys) serialization of the effective, post-resolution
  `ReviewPackReference{id,version}` set, `RepositoryContext.generatedAt`,
  a content fingerprint of `RepositoryContext.criteria`, and
  `KNOWLEDGE_SCHEMA_VERSION` (ADR-0005). Verified deterministic: identical
  inputs twice → identical id; a changed pack set or changed criteria →
  a different id.
- **Dependency validation** — fail-closed: `resolvePolicy` throws, naming
  the exact missing pack/Skill dependency, instead of assembling a partial
  policy (Architecture doc §9).
- **`@debuggatha/review-packs`** — the real `ReviewPack` type (superseding
  its old `@debuggatha/shared`-backed placeholder) plus one trivial,
  clearly-labeled `examplePack`, proving the schema/registry/resolution
  pipeline round-trips end to end. No real stack/concern content — that's
  a later epic (CLAUDE.md).
- **`@debuggatha/policies`** — `assemblePolicy(context, request)`, a thin
  wrapper that calls this package's `resolvePolicy` against a
  `CapabilityRegistry` populated from `@debuggatha/review-packs`.
- **`@debuggatha/skills`** — `reviewArchitecture`/`reviewDiff`'s type
  imports migrated off `@debuggatha/shared`'s legacy `Finding`/
  `ReviewPolicy` onto `@debuggatha/review-engine`'s `Finding` and this
  package's richer `ReviewPolicy`. Still throw-stubs — only the imports
  changed, not the (still unimplemented) logic.

## Deferred (explicitly out of scope for this epic)

- Real stack/concern Review Pack content (Rust, React/TS, Kotlin, OWASP,
  ...) — CLAUDE.md v1 scope names this explicitly as a later epic.
- Filesystem/network pack discovery, a pack marketplace/registry service,
  cross-process/plugin-isolation for Packs — Architecture doc §11.
- Executable/scriptable Rules — Rules stay declarative data.
- Automatic semantic conflict detection between two prose `statement`
  fields (NLP-based contradiction detection) — only explicit `contradicts`
  and the narrow automatic id-collision case below are detected.
- Soft/optional pack dependencies — every dependency is hard, fail-closed.
- A persisted Policy Store / pack version archive — `ReviewPolicy.id` is
  reproducible given the same pack versions are still registered, but
  nothing here guarantees old pack versions stay available forever (the
  Findings ledger's job, per CLAUDE.md, "designed, not yet built").
- A `SkillDescriptor` catalog for `@debuggatha/skills`' two existing Skill
  functions — `@debuggatha/policies`' registry is populated with an empty
  skills list for now (see Risks).
- Anything that actually *runs* a review — `reviewArchitecture`/
  `reviewDiff` still throw; only their type imports changed this epic.

## Technical decisions

- **`ReviewPolicy.rules` is a unified `PolicyRule[]`, not two separate
  arrays.** A Pack `Rule` and a repository `CriteriaRule` are structurally
  different (`CriteriaRule` has no `category`/`appliesTo`/`severity`).
  Rather than exposing both shapes separately and pushing the merge logic
  onto every consumer, `resolvePolicy` normalizes both into `PolicyRule`
  with an `origin` tag (`{kind:"pack",packId,packVersion}` or
  `{kind:"criteria"}`) and `category`/`defaultSeverity` left `undefined`
  for criteria-derived entries — "never fakes certainty" applied to policy
  composition, not just to Findings.
- **`glob`-scoped rules are not filtered during Rule Resolution.**
  `resolvePolicy` only has `RepositoryContext.stack` to check `appliesTo`
  against, not a concrete file list — matching a glob needs the files
  actually in a `ReviewRequest`'s scope, which only exists once a Skill
  executes. A `glob`-scoped `Rule` always survives Rule Resolution
  unfiltered; the final per-file match is left to the Skill that consumes
  it.
- **Conflict detection deliberately narrows ADR-0003's "(category,
  appliesTo) concern key" clause for the Pack-vs-Criteria case.**
  `CriteriaRule` (`@debuggatha/repository-intelligence`, frozen since
  Epic 1) carries neither `category` nor `appliesTo` — there is no way to
  compute a literal concern-key match against it without inventing data
  Epic 1 doesn't provide. This package implements two triggers instead:
  (1) explicit — any pack rule's `contradicts` names another candidate's
  id (pack or criteria, any pack pair); (2) automatic, narrow — a pack
  rule's `id` exactly collides with a repository criteria rule's `id`.
  Pack-vs-pack conflicts are *never* auto-detected by category/scope
  overlap alone (two different packs legitimately sharing a category is
  common and not inherently a conflict) — only the explicit `contradicts`
  path applies there. This is a real, documented narrowing of the
  Architecture doc's literal text, made necessary by the frozen shape of
  `CriteriaRule`.
- **A pack that depends on another pack pulls that pack's rules into the
  effective policy, not just its existence.** ADR-0005 describes the
  "effective set" as "resolved... pairs actually applied," which this
  package reads as: a dependency is not just a presence check, it behaves
  like ESLint's `extends` — the depended-on pack's applicable rules join
  the policy too.
- **`resolvePolicy` verifies `ReviewRequest.requestedPolicyId` rather than
  ignoring it.** There is no Policy Store in v1 (Architecture doc §11), so
  "re-run a specific past policy" can only be honored by recomputing and
  comparing: if `requestedPolicyId` is set and doesn't match the freshly
  computed id, `resolvePolicy` throws, naming both ids — consistent with
  this epic's fail-closed posture elsewhere, rather than silently ignoring
  a caller's stated intent.
- **`ReviewPolicy.id` is truncated to 16 hex characters (64 bits) of a
  SHA-256 digest.** ADR-0005 says "truncated for readability"; 64 bits of
  a cryptographic hash is far more collision resistance than this
  in-process, non-adversarial use case needs.
- **No `npm semver` dependency.** `PackDependency.versionRange` and
  `CapabilityRegistry.getPack`'s optional range only need to support what
  a small, static, in-repo pack catalog actually uses: exact match, `^`,
  `~`, and `*`. A hand-rolled `major.minor.patch` comparator
  (`src/internal/semver.ts`) covers that without a new dependency —
  matching repository-intelligence's own minimal-dependency discipline
  for config parsing.
- **`assembleContext` shallow-freezes only the container it builds, not
  `repository`/`request` recursively.** `repository` is already
  deep-frozen by repository-intelligence; `request` is a caller-owned
  value this function doesn't clone — deep-freezing into it would be a
  surprising side effect on an object this function doesn't own. Contrast
  with `resolvePolicy`, which deep-freezes its own freshly-constructed
  `rules`/`packRefs`/`conflicts` — those arrays are never caller-owned.

## Risks

- **Rule Resolution's precision is bounded by Criteria Resolution's
  precision.** `CriteriaRule` extraction from Markdown is heading-level,
  not semantic (repository-intelligence's own documented risk). Any
  `ReviewPolicy` built partly from `RepositoryContext.criteria` inherits
  that imprecision.
- **The narrowed conflict-detection design (see "Technical decisions")
  means a real gap is wider than ADR-0003's text alone implies.** Two pack
  rules from different packs that address the same concern but were never
  cross-referenced via `contradicts`, and don't happen to share an exact
  rule id with a criteria rule, will silently coexist in `ReviewPolicy.rules`
  with no conflict recorded — this was already an accepted v1 limitation
  in ADR-0003, but this implementation's narrower Pack-vs-Criteria trigger
  (exact id collision only, not a true concern-key match) makes that gap
  slightly larger than a literal reading of the ADR suggests. Revisit if
  `CriteriaRule` ever gains a `category`/`appliesTo`-equivalent.
- **`@debuggatha/policies`' default registry has an empty Skill list.**
  `@debuggatha/skills` doesn't export a `SkillDescriptor` catalog yet
  (only its two throw-stub functions) — a future pack declaring a
  `{ skillId }` dependency will fail closed (correctly, per §9) until that
  catalog exists. Not a bug, but worth knowing before wiring a
  Skill-dependent pack.
- **`compareVersions`/`satisfiesRange` (`src/internal/semver.ts`) is not a
  full semver implementation.** No prerelease/build-metadata handling, no
  `>=`/`<`/range-set syntax beyond `^`/`~`/exact/`*`. Sufficient for a
  static, in-repo v1 catalog; would need a real `semver`-equivalent
  dependency if pack versioning ever needs the full spec.
- **Fail-closed dependency handling (Architecture doc §9) has no escape
  hatch.** If a widely-depended-on pack is removed from the registry
  mid-session, every pack depending on it stops resolving entirely —
  acceptable for a 2–3 pack v1 catalog, worth revisiting as the catalog
  grows.
- **No enforcement that published pack versions stay immutable.** Nothing
  in this package stops a pack author from editing `examplePack` (or a
  future real pack) in place without bumping `version` — a convention,
  not a runtime check (Architecture doc §12).
