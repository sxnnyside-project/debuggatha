# @debuggatha/core

The review domain: what Debuggatha knows and the rules it plays by. It has
no I/O adapters, no orchestration, and no pack content, and depends on
nothing else in the repository. `@debuggatha/engine` runs reviews on top of
it and `@debuggatha/packs` supplies the rules.

## Modules

Each module's architecture, decisions, and limitations are in the [Module reference](#module-reference) below.

| Module | Owns |
| --- | --- |
| [`repository-intelligence`](#corerepository-intelligence) | The immutable `RepositoryContext`: stack, dependencies, documentation, criteria, understanding |
| [`review-engine`](#corereview-engine) | `ReviewRequest`, `ReviewSession`, `Finding`, `Evidence`, `ReviewResult` |
| [`knowledge-system`](#coreknowledge-system) | Review Packs, rules, the capability registry, `resolvePolicy` |
| [`findings-ledger`](#corefindings-ledger) | The persistent findings history and its lifecycle |
| [`repository-memory`](#corerepository-memory) | Long-term suppressions, deviations, conventions, and decisions |
| `testing` | Fixture helpers, exported as `@debuggatha/core/testing` |

Dependencies run in that order: each module may import the ones above it,
never below. Modules import each other by relative path through their
`index.ts`; a name two modules both export is settled explicitly in
`src/index.ts`.

## What should never be implemented here

Filesystem walking for reviews, git access, model calls, or anything that
executes a review. Those belong in `@debuggatha/engine`.

---

## Module reference

## core/repository-intelligence

Produces a `RepositoryContext`: a normalized, immutable snapshot of a repository's stack,
capabilities, dependencies, documentation, and engineering criteria, before any review happens.

This module does not review code. It does not know what a `Finding` or a `ReviewPolicy` is, and it
has no dependency on MCP, VS Code, the CLI, or any model. Every fact it produces is deterministic
and evidence-backed.

### Architecture

One entrypoint, `buildRepositoryContext(rootDir, options?)`, runs independent scanners once and
combines their output:

```text
buildRepositoryContext(rootDir)
├── scanStack           → StackProfile         (languages, frameworks, build systems,
│                                               package managers, runtimes, workspace type,
│                                               platform targets)
├── scanDependencies    → DependencyProfile    (lockfile presence, declared versions of
│                                               recognized frameworks and runtimes)
├── scanDocumentation   → DocumentationProfile (README, CLAUDE.md, ADRs, PRD, roadmap, docs)
├── scanCriteria        → CriteriaProfile      (lint and format config, rule-shaped docs)
├── deriveUnderstanding → UnderstandingProfile (open questions, confidence)
└── deriveCapabilities  → Capability[]         (the flat set rules match against)
```

`scanStack` reads `package.json`, `Cargo.toml`, and `pubspec.yaml` once and hands the parsed result
to `scanDependencies`, so nothing is parsed twice. The profiles are assembled into one
`RepositoryContext`, deep-frozen at runtime (not just `readonly` in types), and optionally cached.

Every scanner returns `{ profile, filesRead, dirsListed }` so the orchestrator can fingerprint
exactly what it read: every directory it listed and every file it opened, each with an
`mtimeMs`/`size` stamp. `RepositoryContextCache` is injectable (no global singleton); on the next
call for the same root the fingerprint is re-checked with stat calls, and the cached context is
returned unchanged (tests assert referential equality).

### Repository capabilities

`RepositoryContext.capabilities` (`Capability[]`, from `deriveCapabilities`) is the machine-matchable
contract Review Pack resolution consumes. `StackProfile` remains the human-readable representation.

```text
scanStack → StackScan (StackProfile + parsed manifests)
        │
        ▼
deriveCapabilities(rootDir, rootEntries, stackScan)
        │  normalizes each StackSignal into a lowercase alphanumeric id
        │  ("React" -> "react", "Next.js" -> "nextjs", "ASP.NET Core" -> "aspnetcore")
        │  splits the combined "JavaScript/TypeScript" signal into two independent capabilities
        │  adds what StackProfile does not track: PHP/Laravel (composer.json), a VS Code
        │  extension platform (package.json engines.vscode), tooling (biome, eslint, vitest
        │  config files), and repository characteristics (monorepo, package-based,
        │  feature-based, layered)
        ▼
Capability[] { id, kind, confidence, evidence, origin }
        │
        ▼
resolvePolicy (core/knowledge-system) matches each Rule's requires-language / requires-framework
scope against this flat id set
        │
        ▼
summarizeCapabilities(capabilities) → readable summary, grouped by kind
```

**Additive, never mutually exclusive.** A repository with TypeScript, JavaScript, React, Bun,
Turborepo, and Tauri gets six independent `Capability` entries, not one chosen "stack" label. That
is what lets a polyglot repository resolve every applicable Review Pack at once.

**JavaScript and TypeScript are separate.** A `package.json` gives a `javascript` capability
unconditionally; `typescript` is added when `tsconfig.json` is present (confidence `high`) or
`typescript` is a declared dependency (confidence `medium`). Packs declare separate
`requires-language` scopes for each, so both must be independent ids.

**Unknown is preferable to incorrect.** Every capability carries its own `confidence`. A capability
with no supporting evidence is never emitted: absence is the "unknown" signal.

### Capabilities of the module

- **Stack Detection**: languages, frameworks, build systems, package managers, runtimes, workspace type, and platform targets, each with an `Evidence[]` trail. Ambiguous filenames are resolved by content (a `Cargo.toml` is "Tauri" only if it depends on `tauri`; a `pubspec.yaml` is "Flutter" only if it depends on `flutter`).
- **Dependency Context**: lockfile presence and declared version constraints for the frameworks and runtimes Stack Detection recognized (metadata, not a dependency audit).
- **Documentation Context**: README, CLAUDE.md, CONTRIBUTING.md, ARCHITECTURE.md, ROADMAP.md, and a bounded walk of `docs/` classifying ADRs, PRDs, and roadmaps by path convention, with a mechanically generated summary sentence.
- **Criteria Resolution**: JSON-shaped configs (`.eslintrc(.json)`, `biome.json`, `.prettierrc(.json)`) parse into structured rules; `.editorconfig` gets an INI parser; `rustfmt.toml` a flat `key = value` extractor; rule-shaped Markdown (`ARCHITECTURE.md`, `CONTRIBUTING.md`, `CLAUDE.md`, `*_CRITERIA.md`) is structured at heading granularity.
- **Repository Understanding**: a pure function over the profiles. It asks a question only when a concrete gap exists (no language, no runtime marker, no documentation, no criteria), and confidence degrades as questions accumulate.
- **Snapshot and cache**: the immutable `RepositoryContext` and its fingerprint-based cache.

### Technical decisions

- **No YAML or TOML parser dependency.** `.editorconfig` gets a real INI-style parser; `rustfmt.toml` a bounded flat extractor. A full parser for one or two config shapes was not worth the weight.
- **The cache is injectable.** Without a `cache` option `buildRepositoryContext` always rescans: correctness over convenience.
- **The fingerprint tracks directory listings, not just file stamps.** Otherwise a new `Cargo.toml` in a repository that had none would go unnoticed, since there is no previous stamp to compare.
- **Markdown criteria are structured at heading granularity only.** Extracting "## No unwrap() in production code" as a `CriteriaRule` says a rule-shaped section exists and names it; it does not claim to understand the prose beneath.
- **Dependency Context tracks a fixed allowlist** (`react`, `next`, `vue`, `svelte`, `electron`, the two `@tauri-apps/*` packages), not the whole tree.
- **Platform targets are additive-only.** A framework positively implies web, desktop, or mobile; absence implies nothing, so a backend-only service gets an empty list instead of a guessed one.

### Limitations

- `.eslintrc` without an extension may be YAML or a JS module; only `JSON.parse` is attempted, so a YAML-flavored file is skipped without error. The same applies to a YAML `.prettierrc`.
- `detekt.yml` is recorded as present but never parsed.
- `rustfmt.toml` parsing loses nested tables and arrays silently.
- Heading-level extraction cannot tell a real rule from an unrelated heading ("## Acknowledgements" becomes a `CriteriaRule`). Consumers must treat `description` as the name of a section that might be a rule.
- `platformTargets` cannot positively identify backend or library projects.
- The in-memory cache does not persist across processes, so every CLI run or server start scans cold. A disk-backed cache raises correctness questions (a stale cache surviving a `git checkout`).
- Capability derivation covers a subset of the frameworks in the packs: ASP.NET Core has no detector (`.csproj` files are not scanned), so a rule scoped `requires-framework: "aspnetcore"` never resolves. PHP and Laravel detection reads `composer.json` inside `capabilities.ts` rather than through `scanStack`.
- Repository-characteristic detection is directory-name based and intentionally shallow (one level of `src/*`); `engine/analysis-engine`'s Module Boundaries is the thorough version.
- Lockfile content (resolved or transitive versions) is not parsed.

---

## core/review-engine

The review domain: what a request, a session, a finding, and a result are, independent of who or
what produces them.

This module does not talk to a model, does not know what MCP or the CLI are, and does not implement
Review Packs or Policies; it references them by id. It turns a request plus a `RepositoryContext`
(from `core/repository-intelligence`) into structured objects that any review source (a model, a
deterministic analyzer, a custom skill, or a human) can populate the same way.

### Architecture

```text
ReviewRequest ────┐
                  ├─► createReviewSession ──► ReviewSession (status: "requested")
RepositoryContext ┘                              │
                                    transitionSession (enforced state machine)
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

Findings are constructed through `createFinding`, which enforces the two non-negotiable invariants,
at least one location and at least one piece of evidence, before a `Finding` can exist. A
`ReviewResult` can only be built from a `"completed"` session. Both are runtime checks, not just
types: a finding that violates "never opine without evidence" should be impossible to construct.

### Domain model

| Type | Role |
|---|---|
| `ReviewRequest` | One request shape for diff, file, and workspace review, discriminated by `scope.kind`. |
| `ReviewSession` | The process record: id, timestamps, the `RepositoryContext` it ran against, selected policy and pack references, lifecycle `status`, `execution` metadata. |
| `ReviewLifecycleStatus` | `requested → prepared → running → (completed \| failed \| cancelled)`, enforced by `canTransition`/`transitionSession`. |
| `ReviewSource` | `ai-provider \| deterministic-analyzer \| custom-skill \| human-review`: the domain never assumes a model is involved. |
| `Finding` | Title, explanation, `Severity`, `Confidence` (independent), `Category` (open string), `locations`, `evidence`, `appliedPolicy` and `originatingPack` references, `recommendations`. |
| `Evidence` | Discriminated union: `documentation \| criteria \| review-pack-rule \| framework-convention \| language-convention \| code`, plus `external-analyzer` and model-raised variants. |
| `Recommendation` | A structured fix (`action`, `summary`, optional target location and exact edit), not a prompt. |
| `ReviewResult` | `summary`, `findings`, and result-level `recommendations`, linked to its session by `sessionId`. |
| `ReviewSummary` | Counts by severity and category, applied packs and policies, duration; computed once by `summarizeFindings`. |
| `ReviewPackReference` (`{ id, version }`), `ReviewPolicyReference` (`{ id }`) | Stand-ins: this module never imports the packs or `engine/policies`. See ADR-0002 and ADR-0005. |

### Technical decisions

- **`affectedFiles` is derived, not stored.** `Finding.locations` (file plus optional line range) is the single source of truth, so the two cannot drift.
- **`ReviewSession` and `ReviewResult` are separate objects**, linked by `sessionId`. A session is process bookkeeping; a result is the deliverable. Consumers can list sessions without loading results.
- **`Finding.recommendations` and `ReviewResult.recommendations` are both real.** A finding's are fixes for that finding; a result's are review-level (for example a repository-wide convention several findings point to).
- **`Category` is `KnownCategory | (string & {})`**, not a closed union, so adding a category is not a breaking change while autocomplete keeps the known ones.
- **`Severity` and `Confidence` are unrelated types** and no function converts between them. The rubric for each is documented on the types.
- **`ReviewDepth` is defined locally** rather than imported from another package.
- **Construction is validated and transitions are pure.** `createFinding`, `createReviewRequest`, and `createReviewResult` throw on invalid input; `transitionSession` returns a new session.
- **There is no named lifecycle wrapper** (`startSession`, ...): `transitionSession` is the one primitive.

### Limitations

- `transitionSession`'s `patch` argument is blunt: a caller can set `execution.error` while completing, or omit it while failing. A stricter version could require `error` when `to === "failed"`.
- `ReviewScope`'s `"diff"` variant is not validated (an empty or malformed `base` ref passes), because this module never resolves a scope against a real repository. `engine` validates refs where it runs git.

---

## core/knowledge-system

Defines what a Skill, a Review Pack, and a Review Policy are, how they are discovered and combined,
and how a `ReviewPackReference` / `ReviewPolicyReference` (review-engine stand-ins) become real
content a Review Skill can execute against and a `Finding` can cite. See
[docs/knowledge-system/ARCHITECTURE.md](../../docs/knowledge-system/ARCHITECTURE.md) and
[ADR-0001 to ADR-0005](../../docs/adr).

This module does not talk to a model, does not know what MCP, the CLI, or VS Code are, and ships no
pack content. It depends on `core/repository-intelligence` (`RepositoryContext`, `CriteriaRule`) and
`core/review-engine` (`Evidence`, `Category`, `Severity`, the reference types), never the reverse.

### Architecture

```text
CapabilityRegistry (Skills + Packs, injectable, in-memory)
        │ resolveDependencies(requestedPackIds)
        ▼
resolvePolicy(RepositoryContext, ReviewRequest, CapabilityRegistry)
        │  1. resolve requested packs + transitive dependencies (fail-closed)
        │  2. filter each pack's Rules by appliesTo against the repository's capabilities
        │  3. union survivors with RepositoryContext.criteria.rules
        │  4. detect and resolve conflicts (ADR-0003), record ConflictRecord[]
        │  5. hash the effective inputs into a deterministic id (ADR-0005)
        ▼
ReviewPolicy { id, rules, packRefs, conflicts }  (frozen)
        │
        ▼
assembleContext(RepositoryContext, ReviewRequest, ReviewPolicy | undefined)
        ▼
SkillContext { repository, request, policy }  (the one shape every Skill receives)
```

`validateReviewPack` is a separate pure structural check run at authoring and test time; it is not
part of the `resolvePolicy` pipeline.

### Domain model

| Type | Role |
|---|---|
| `SkillDescriptor` (`id`, `tier`, `description`) | The cheap index entry for a Skill; `tier` is `core`, `review`, or `analysis`. |
| `Rule` | Atomic, declarative statement owned by a pack: `appliesTo` scopes it, `knowledgeRefs` cites at least one `KnowledgeEntry`, `contradicts` opts in to conflict detection. |
| `RuleScope` | `always \| requires-framework \| requires-language \| glob`. |
| `KnowledgeEntry` | The domain expertise a Rule derives from, with required `limitations`. |
| `ReviewPack` | `id`, `version` (semver), `kind` (`stack \| concern`), `rules`, `knowledge`, `dependsOn`. |
| `PackDependency` | A hard dependency on a pack (`{ packId, versionRange }`) or a Skill (`{ skillId }`). |
| `CapabilityRegistry` | `listSkills`, `listPacks`, `getPack`, `resolveDependencies`; `createInMemoryRegistry` is the implementation (ADR-0004). |
| `PolicyRule` | The unified shape `ReviewPolicy.rules` carries: a pack `Rule` or a repository `CriteriaRule`, tagged with `origin`. |
| `ConflictRecord` | `{ winningRuleId, losingRuleId, reason }`. |
| `ReviewPolicy` | `{ id, rules, packRefs, conflicts }`; `id` is a deterministic hash. |
| `SkillContext` | `{ repository, request, policy }`; Core Skills get `policy: undefined`. |
| `resolvePolicy`, `assembleContext`, `validateReviewPack` | The pure functions described above. `validateReviewPack` throws one aggregated error naming every structural issue. |

### Technical decisions

- **`ReviewPolicy.rules` is one unified `PolicyRule[]`.** A pack `Rule` and a repository `CriteriaRule` differ structurally (`CriteriaRule` has no `category`, `appliesTo`, or severity), so `resolvePolicy` normalizes both with an `origin` tag and leaves `category` and `defaultSeverity` undefined for criteria-derived entries, rather than pushing the merge onto every consumer.
- **`appliesTo` matches the repository's capability set**, not exact stack strings, so a `requires-language: typescript` rule survives for a TypeScript repository however the stack label is spelled.
- **`glob`-scoped rules are not filtered during resolution.** Matching a glob needs the concrete files in a request's scope, which exists only when a Skill runs; a glob-scoped rule survives and the Skill does the per-file match.
- **Conflict detection narrows ADR-0003's concern key for pack-versus-criteria.** `CriteriaRule` has neither `category` nor `appliesTo`, so a literal concern-key match cannot be computed without inventing data. Two triggers apply: an explicit `contradicts` naming another candidate's id, and an exact id collision between a pack rule and a criteria rule. Pack-versus-pack conflicts are detected only through `contradicts`, since two packs sharing a category is common and not inherently a conflict.
- **A dependency pulls the depended-on pack's applicable rules into the policy**, like ESLint's `extends`, not just a presence check.
- **`resolvePolicy` verifies `requestedPolicyId`.** With no policy store, "re-run a past policy" is honored by recomputing and comparing; a mismatch throws, naming both ids.
- **`ReviewPolicy.id` is 16 hex characters (64 bits) of a SHA-256 digest**, ample for non-adversarial in-process identity.
- **No `semver` dependency.** A small `major.minor.patch` comparator (`src/internal/semver.ts`) supports exact match, `^`, `~`, and `*`, which is what an in-repo catalog uses.
- **`assembleContext` shallow-freezes only the container it builds.** `repository` is already deep-frozen and `request` is caller-owned; `resolvePolicy` deep-freezes the arrays it constructs itself.

### Limitations

- Resolution precision is bounded by Criteria Resolution: criteria extracted from Markdown are heading-level, so a repository-derived rule may be a section title.
- Undeclared conflicts coexist: two pack rules on the same concern that were not cross-referenced, and do not collide by id with a criteria rule, both stay in `ReviewPolicy.rules`.
- `engine/policies` registers an empty Skill list, so a pack declaring a `{ skillId }` dependency fails closed until Skills are registered as descriptors.
- `compareVersions` and `satisfiesRange` are not full semver: no prerelease or build metadata, and no range syntax beyond `^`, `~`, exact, and `*`.
- Fail-closed dependencies have no escape hatch; if a widely depended-on pack is removed, its dependents stop resolving.
- Nothing enforces that a published pack version stays immutable.
- Not built: filesystem or network pack discovery, executable rules, semantic conflict detection, optional dependencies, and a persisted policy store.

---

## core/findings-ledger

The historical record of every finding produced for a repository: persistence, traceability, and
lifecycle across many review sessions.

This module is not a database. It knows nothing about MCP, the CLI, VS Code, or any model, and does
no cloud sync, collaboration, or telemetry. It depends only on `core/repository-intelligence`
(`RepositoryContext`) and `core/review-engine` (`Finding`, `ReviewResult`, `Severity`, `Category`),
never on `core/knowledge-system`: it needs only the `ReviewPackReference` and
`ReviewPolicyReference` stand-ins review-engine already carries.

### Architecture

```text
ReviewResult ───────┐
                    ├─► synchronizeReviewResult(ledger, result, context, scope)
RepositoryContext ──┘         │
                              │ for each Finding: defaultFindingMatcher
                              │   -> new             => createLedgerEntry (status: open)
                              │   -> matched, active => refreshLedgerEntry (no history event)
                              │   -> matched, closed => transitionFinding(..., "reopened")
                              │ for each untouched active entry within `scope`:
                              │   => transitionFinding(..., "resolved")
                              ▼
                        Ledger { entries: LedgerEntry[] }  (frozen)
                              │
             ┌────────────────┼────────────────┐
             ▼                ▼                ▼
     updateFindingStatus  summarizeLedger  serializeLedger / saveLedger
     (manual transitions)                   → .debuggatha/ledger.json
```

Every operation returns a new `Ledger`; nothing mutates a `Ledger`, `LedgerEntry`, or `HistoryEvent`
in place.

### Domain model

| Type | Role |
|---|---|
| `Ledger` | `{ schemaVersion, repositoryRoot, entries, createdAt, updatedAt }`: one repository's history. |
| `LedgerEntry` | One tracked issue across its lifetime: stable `id` (distinct from `latestFinding.id`, which changes every run), `fingerprint`, `status`, append-only `history`, `latestFinding`, `review`. |
| `FindingFingerprint` | `{ file, ruleId, category, contentAnchor }`: finding identity. Never keys on line number alone. |
| `FindingLifecycleStatus` | `open \| acknowledged \| resolved \| dismissed \| reopened`, enforced by `canTransitionFinding`. |
| `HistoryEvent` | `{ timestamp, previousState, newState, origin, comment }`: one immutable record of a transition. |
| `TransitionOrigin` | `{ kind: "review"; sessionId }` or `{ kind: "manual"; actor }`. |
| `ReviewAssociation` | `{ createdBySessionId, lastUpdatedBySessionId, repositorySnapshot, appliedPolicy, appliedPacks }`: traceability. |
| `RepositorySnapshotRef` | `{ generatedAt, fingerprint }`: a reference to the repository state a finding was produced against. |
| `FindingMatcher` | `(candidate, candidateEntries) => MatchOutcome`: pluggable; `defaultFindingMatcher` is the built-in strategy. |
| `LedgerSummary` | `{ countsByStatus, activeBySeverity, activeByCategory }`. |

### Capabilities

- **Finding identity**: `computeFindingFingerprint` from `Finding.evidence` (rule evidence for `ruleId`, a hash of the code excerpt for `contentAnchor`).
- **Lifecycle**: all five states, `canTransitionFinding` (explicit table), `transitionFinding` (pure, throws on an illegal transition).
- **History**: append-only; every transition adds exactly one event and never edits an earlier one.
- **Matching and sync**: `defaultFindingMatcher`, `synchronizeReviewResult`.
- **Persistence**: `serializeLedger`/`deserializeLedger` (deterministic, versioned, fail closed on an unrecognized version) and `loadLedger`/`saveLedger` (`.debuggatha/ledger.json`, written atomically).
- **Public API**: `listEntries`, `getEntry`, `getHistory`, and `updateFindingStatus`, the only sanctioned way to change a status.

### Technical decisions

- **`SyncScope` is supplied by the caller**, never inferred from `ReviewRequest.scope`. Knowing which files a diff touched needs git knowledge this module deliberately lacks, so `synchronizeReviewResult` takes an explicit `{ kind: "workspace" }` or `{ kind: "files"; files }`.
- **Matching candidates are every entry, not just active ones.** Matching a resolved or dismissed entry is what reopens it; only the auto-resolve pass is limited to active entries.
- **A multi-location finding is fingerprinted on its first location.** Documented, not silently assumed (see Risks).
- **`LedgerEntry.id` is random, not derived from the fingerprint**, keeping identity assignment and matching two independent concerns.
- **`updateFindingStatus` enforces the same state machine as an automated transition.** There is no privileged manual override; `TransitionOrigin` records who, not a different set of allowed moves.
- **No persistence dependency.** Plain `JSON.stringify`/`JSON.parse` with a deterministic key sorter.
- **`countsByStatus` covers the full lifecycle; `activeBySeverity` and `activeByCategory` cover only open, acknowledged, and reopened**, because a resolved finding's severity is not actionable.

### Persistence format

`.debuggatha/ledger.json` is one UTF-8 JSON file, 2-space indented, with a trailing newline and keys sorted at every level, so the same `Ledger` always serializes to the same bytes.

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
      "latestFinding": { "...": "the full Finding object" },
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

A single file is simpler to write atomically. One file per entry would merge better in a team setting and is a possible change if that becomes a real cost.

### Limitations

- A multi-file finding's identity ignores every location after the first, so two different multi-file findings with the same first `(file, ruleId)` can match each other. Most findings are single-location.
- The matcher's `(file, category)` fallback, used when no `ruleId` is available, is coarse: unrelated rule-less findings in the same file and category can merge.
- `RepositorySnapshotRef.fingerprint` covers content (`stack`, `dependencies`, `documentation`, `criteria`) but not `generatedAt` or `hasGit`, so it identifies the same content, not the same scan.
- The ledger keeps the most recent `Finding` plus the status trail, not every `Finding` ever produced.
- `migrateLedger` recognizes only `CURRENT_SCHEMA_VERSION` and fails closed on anything else; it has no migration steps yet.
- The ledger only grows: there is no pruning of old resolved or dismissed entries.

---

## core/repository-memory

Long-term, repository-specific engineering knowledge that accumulates across review sessions:
accepted deviations, known false positives, architectural exceptions, conventions, review-history
notes, and engineering decisions.

**Not a conversation history.** No prompts, no generated prose, and no raw model output are ever
persisted, only structured engineering knowledge. The objective is to remember engineering
decisions.

### Lifecycle

```text
detected → suggested → confirmed → active → deprecated → archived
```

`canTransitionMemory` / `LEGAL_TRANSITIONS` is the single source of truth for legal transitions,
the same pattern as the ledger's `canTransitionFinding`.

- **`detected`**: noticed automatically; nobody has looked at it.
- **`suggested`**: surfaced to a person as worth remembering.
- **`confirmed`**: a person said yes. Only `confirmMemory`, gated on a `{ kind: "user" }` origin, produces this transition.
- **`active`**: influences reviews. The only status `filterSuppressedFindings` ever consults.
- **`deprecated`**: stopped influencing reviews but kept for re-confirmation (for example the repository changed enough that the item may no longer hold).
- **`archived`**: terminal; kept for audit.

`reject` (declining a `detected` or `suggested` item) goes straight to `archived`: it was never true,
so there is nothing to deprecate. `archiveMemory` reaches `archived` from any non-archived state.
`removeMemoryItem` is a harder action that deletes the item entirely, because "stop remembering
this" and "this was true but no longer applies, keep the history" are different intents.

### Persistence

`.debuggatha/memory.json`, next to the ledger. Deterministic (sorted keys), versioned
(`schemaVersion`; an unrecognized version fails closed), human-readable and diffable in a pull
request. `migrateMemoryStore` checks version equality only; it is where a migration step is added
the first time the schema changes.

### Confidence

Three levels (`MemoryConfidence`), separate from any extraction confidence, because memory is about
decisions rather than extracted facts:

- **`inferred`**: a deterministic pass's best guess.
- **`documented`**: backed by something written down (a CONTRIBUTING rule, a CODEOWNERS entry).
- **`user-confirmed`**: a person explicitly said yes. Never produced automatically; the only path is `confirmMemory` with a `{ kind: "user" }` origin.

### Items and review enrichment

Six categories (`MemoryCategory`), each its own interface: `accepted-deviation`, `suppression`,
`exception`, `convention`, `review-history`, `decision`. Every item carries `id`, `status`,
`confidence`, `history`, and, required, a `rationale` and `evidence: MemoryEvidence[]`, so anyone can
see why Debuggatha remembers something.

`suppression`, `accepted-deviation`, and `exception` items match a `Finding` the way the ledger
recognizes "the same finding": `matching.ts` reuses `computeFindingFingerprint` rather than
re-deriving identity. `review-history` items reference a `LedgerEntry.id`; this module never stores
its own copy of a finding's history.

### Integration

`filterSuppressedFindings(findings, store)` runs inside `executeReview`: each review loads
`.debuggatha/memory.json` (an empty store when the file is absent) and filters findings before they
reach the ledger, with no caller changes. Only `active` items are consulted, so an unconfirmed
suggestion can never hide a real finding. Every suppressed finding is reported in
`ReviewPipelineOutput.suppressedFindings`, never dropped. The CLI's `memory` commands and the MCP
`suppress_finding` tool are the adapter surfaces over this API.

### Not built

- Automatic `detected` to `suggested` promotion: `suggestMemory` must be called by whatever detects an item.
- Recurring-pattern detection: a `review-history` note can record recurrence, but nothing creates one from a finding recurring across runs.
- Deriving `decision` items from documentation, such as an ADR.
