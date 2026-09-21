# Knowledge System — Architecture

The Knowledge System (`core/knowledge-system`) defines what a Skill, a Review Pack, a Rule, and a Review Policy are, how a registry answers "what exists and what does it need," and how a review's policy is assembled from packs and the repository's own criteria. It builds on [Repository Intelligence](../../packages/core/README.md#corerepository-intelligence), which produces `RepositoryContext` (what a repository *is*), and the [Review Engine domain](../../packages/core/README.md#corereview-engine), which defines `ReviewRequest`, `Finding`, and `Evidence` (what a review *produces*). Decisions are recorded in [ADR-0001 to ADR-0005](../adr).

## 1. Design influences

| System | What it taught |
|---|---|
| Language Server Protocol | Describe capability up front, without executing anything. |
| Biome plugins | Declarative content, registered by configuration; start narrower than a scripting language. |
| Terraform providers | Separate schema from content: the plugin ships a schema-conformant bundle, the engine that interprets it lives elsewhere. Determinism of plan and apply. |
| Open Policy Agent bundles | Overlapping or conflicting policy is caught and combined by an explicit, documented strategy, not merged quietly. |
| ESLint flat config | Additive layering of configuration; its implicit last-wins conflict handling is what Debuggatha declines to copy (ADR-0003). |

## 2. Data flow

```text
RepositoryContext ───────►┌────────────────────────┐◄─── ReviewRequest
  (criteria, capabilities) │    Rule Resolution      │     (requestedPackIds / requestedPolicyId)
                           │ resolvePolicy(...)      │
CapabilityRegistry ───────►└───────────┬────────────┘
  (packs, skills)                      │ ReviewPolicy (frozen: rules, packRefs, conflicts)
                                       ▼
                           ┌────────────────────────┐
                           │   Context Assembly      │
                           │ assembleContext(...)    │
                           └───────────┬────────────┘
                                       │ SkillContext
                                       ▼
                                Skill (engine/skills)
                                       │
                                       ▼
                                  Finding[] citing a Rule or a CriteriaRule
```

Every step is pure and produces an immutable value: nothing mutates a `RepositoryContext`, a `ReviewPack`, or a previously produced `ReviewPolicy`.

## 3. Relationship to the review domain

| Review domain type | Knowledge System type | Relationship |
|---|---|---|
| `RepositoryContext.criteria` (`CriteriaRule[]`) | `ReviewPolicy.rules[]` | Rule Resolution merges the repository's own criteria with a pack's `Rule[]`, tagging each with provenance. |
| `ReviewPackReference { id, version }` | `ReviewPack` | The reference is the pointer; the registry resolves it to the pack (ADR-0002). |
| `ReviewPolicyReference { id }` | `ReviewPolicy` | Same relationship; the id is a deterministic hash (ADR-0005). |
| `Evidence` variants `review-pack-rule` and `criteria` | `Rule.id` / `CriteriaRule.id` | These are the join keys a `Finding` uses to cite a rule or a repository criterion. |
| `ReviewSession.selectedPolicy` / `selectedPacks` | Output of Rule Resolution | The orchestrator calls Rule Resolution and stores the reference; the Knowledge System never constructs a session. |
| `ReviewSource` | `SkillDescriptor.tier` | Orthogonal axes: what produced a finding, and which tier (`core`, `review`, `analysis`) a Skill belongs to. |

## 4. Components

**Capability Registry** answers "what exists and what does it need," and nothing else. It holds `SkillDescriptor[]` and `ReviewPack[]`, resolves a pack's declared dependencies, and reports the missing ones. It never executes anything and never sees a `RepositoryContext`. It is injectable rather than a global singleton (ADR-0004).

**Rule Resolution** is a pure function `(RepositoryContext, requested pack ids, CapabilityRegistry) → ReviewPolicy`. It keeps each pack's rules whose `appliesTo` matches the repository's capabilities (a flat, additive set of ids such as `typescript`, `react`, `bun`), merges them with the repository's criteria, detects conflicts, and returns a frozen policy. It throws if a requested pack's dependency cannot be resolved.

**Context Assembly** is a pure function `(RepositoryContext, ReviewRequest, ReviewPolicy | undefined) → SkillContext`. `SkillContext` is the one shape every Skill receives. Core Skills receive it with no policy (they run before any pack is selected); Review and Analysis Skills receive a resolved one.

**Review Packs** (`@debuggatha/packs`) are content, not logic: `ReviewPack` values conforming to the schema this module defines, with no algorithms.

**Policies** (`engine/policies`) wires a registry populated from the packs to `resolvePolicy` and exposes `assemblePolicy` as the call site the rest of the system uses.

**Skills** (`engine/skills`) execute. A Review Skill is a function `(SkillContext) → Finding[]`; an Analysis Skill returns structural facts, not findings.

## 5. Rules and Knowledge

```ts
interface Rule {
  id: string;                  // stable within its pack
  packId: string;              // the owning pack, kept for cheap citation
  statement: string;           // one atomic, individually citable sentence
  category: Category;
  appliesTo: RuleScope;        // when the rule is relevant
  defaultSeverity: Severity;   // a starting point; a Skill may adjust per instance
  knowledgeRefs: string[];     // at least one KnowledgeEntry id: no source, no finding
  contradicts?: string[];      // rule ids this rule is known to conflict with
}

type RuleScope =
  | { kind: "always" }
  | { kind: "requires-framework"; framework: string }
  | { kind: "requires-language"; language: string }
  | { kind: "glob"; pattern: string };

interface KnowledgeEntry {
  id: string;
  title: string;
  body: string;                 // the domain expertise a Rule derives from
  externalRefs?: string[];      // e.g. an OWASP or WCAG URL
  limitations: string[];        // when a rule citing this entry is wrong; never empty
}
```

Rules are declarative data, not executable code. A Rule cannot decide "this file violates me"; that judgment is the Skill's, with the Rule as a citable constraint. Packs are therefore safe to load without a code-execution trust boundary, and the domain is testable without a sandbox. [PACK_SPEC.md](../PACK_SPEC.md) is the authoring standard.

## 6. Lifecycle: defined to applied

1. **Defined**: a `ReviewPack` is authored in `@debuggatha/packs` and checked by `validateReviewPack` (every `knowledgeRefs` entry resolves, every `dependsOn` is well-formed, ids are unique, limitations are present).
2. **Registered**: the pack is added to a `CapabilityRegistry`.
3. **Requested**: `ReviewRequest.requestedPackIds` (or `requestedPolicyId`, to re-run a policy) names what a session wants.
4. **Resolved**: Rule Resolution turns the request and the `RepositoryContext` into a frozen `ReviewPolicy`, checking dependencies and recording conflicts.
5. **Assembled**: Context Assembly combines the policy, the context, and the request into a `SkillContext`.
6. **Applied**: a Skill runs and produces findings, each citing the `Rule` (`review-pack-rule` evidence) or `CriteriaRule` (`criteria` evidence) that justified it, stamped with `appliedPolicy` and `originatingPack`.

## 7. Composition and conflicts

**Composition.** A session may select several packs. The default is a union: every rule from every requested pack whose `appliesTo` matches, plus every criterion from `RepositoryContext.criteria`. No pack owns a review.

**Conflicts.** Two rules conflict when they name each other in `contradicts`, or when a pack rule and a repository criterion share a concern key, an exact match on `(category, appliesTo)`, and make opposite claims. Nothing attempts to detect contradictions between prose statements.

**Resolution.** The repository's own criterion always beats a pack rule (the repository's stated intent is ground truth). Between two packs, the one named earlier in `requestedPackIds` wins. The losing rule is never silently dropped: `ReviewPolicy.conflicts` records both ids, which won, and why (ADR-0003).

## 8. Versioning

Packs are semver-versioned. `ReviewPackReference { id, version }` is a structured field, and `requestedPackIds` stays bare ids; the registry resolves the concrete version at Rule Resolution time (ADR-0002). `ReviewPolicy.id` is a deterministic hash of the sorted `{ id, version }` pairs actually applied, the criteria fingerprint, and a `KNOWLEDGE_SCHEMA_VERSION` (ADR-0005), so the same repository state and pack versions produce the same id.

A finding's `appliedPolicy` and `originatingPack` keep pointing at the exact pack version that produced them, provided a published pack version is treated as immutable: a content or behavior change needs a new version. That is a convention, not a runtime check. Keeping old pack content retrievable is the ledger's concern, not the Knowledge System's.

## 9. Dependencies

`ReviewPack.dependsOn` is a list of `{ packId, versionRange }` or `{ skillId }`. Missing dependencies are fail-closed: if a requested pack's dependency is not registered, Rule Resolution throws instead of assembling a partial policy, in line with "never fakes certainty." There are no optional dependencies.

## 10. Adding a Review Pack

1. Author a `ReviewPack` in `packages/packs/src/packs/<name>.ts` following §5 and the spec.
2. Run `validateReviewPack` in the pack's tests.
3. Add it to the exported array in `packages/packs/src/index.ts`.

No change is needed in `review-engine`, `policies`, `skills`, `mcp`, or `cli`: a well-formed pack is immediately requestable, because those packages depend on the schema, not on any pack's content.

## 11. Deliberately not built

- **Filesystem or network pack discovery.** Every pack ships in this repository's release. A loader and sandbox for third-party pack code would be speculative complexity.
- **Executable rules.** Declarative data avoids a code-execution trust boundary.
- **Automatic semantic conflict detection.** Only conflicts a pack author declares are caught.
- **Optional dependencies.** Every dependency is hard; a "use if available" mode adds a second failure path with no evidence it is needed.
- **Plugin process isolation.** Unnecessary while every pack is in-tree.
- **A persisted policy store or pack archive.** A policy is reproducible while its pack versions remain registered.

## 12. Risks and limitations

- **Resolution precision is bounded by Criteria Resolution.** Criteria extracted from Markdown are heading-level, not semantic, so a criterion derived from a repository's docs may be a section title rather than a verified constraint. Skills must treat repository-derived entries with that caution.
- **Fail-closed dependencies have no escape hatch.** If a widely depended-on pack becomes unavailable, every pack that depends on it stops resolving.
- **Pack immutability is not enforced.** Nothing stops a pack's rules changing without a version bump; a CI check comparing rules against the declared version would close the gap.
- **Undeclared conflicts coexist.** Two rules that should conflict but were not cross-referenced can produce contradictory findings without a flag.

## 13. Boundaries

`core/knowledge-system` depends on `core/repository-intelligence` and `core/review-engine`, never the reverse. `@debuggatha/packs` (content), `engine/policies` (wiring), and `engine/skills` (execution) depend on it. It consumes the review domain's types as they are and adds none to them.
