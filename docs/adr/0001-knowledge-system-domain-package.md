# 0001. `core/knowledge-system` owns the Pack and Policy domain logic

## Status

Accepted

## Context

`core/repository-intelligence` and `core/review-engine` are pure domain modules: no dependency on MCP, the CLI, VS Code, or any model. Pack and Policy concepts (`SkillDescriptor`, `ReviewPack`, `Rule`, `KnowledgeEntry`, a `CapabilityRegistry`, Rule Resolution, Context Assembly, `ReviewPolicy`, conflict handling) are domain logic, not content and not orchestration. Mixing them into the pack content package or into the policy wiring would put logic and content in one package, which makes both harder to version and test independently.

## Decision

`core/knowledge-system` is the pure-domain module for Packs and Policies. It:

- Depends on `core/repository-intelligence` (`RepositoryContext`, `CriteriaRule`) and `core/review-engine` (`Evidence`, `Category`, `Severity`, `ReviewPackReference`, `ReviewPolicyReference`).
- Is depended on by `@debuggatha/packs` (which conforms its content to `ReviewPack`), `engine/policies` (which wires a populated `CapabilityRegistry` to `resolvePolicy`), and `engine/skills` (which consumes `SkillContext` and cites `Rule` and `KnowledgeEntry` ids in finding evidence).
- Has no dependency on any adapter or model provider.

`@debuggatha/packs` stays content-only (a library of `ReviewPack` values); `engine/policies` stays a thin call site.

## Consequences

- Rule Resolution, conflict detection, and dependency resolution are unit-testable with small in-memory fixtures, without real pack content.
- The dependency chain is `repository-intelligence` → `review-engine` → `knowledge-system`, re-exported by `core`'s façade.

## Alternatives considered

- **Put the logic in `engine/policies`.** Rejected: every consumer would import a policy-assembly package just for types, and testing the algorithm would require whatever pack content happens to ship.
- **Fold the types into `core/review-engine`.** Rejected: `review-engine` deliberately references packs and policies by id only; reopening that boundary would couple the review domain to pack content.
