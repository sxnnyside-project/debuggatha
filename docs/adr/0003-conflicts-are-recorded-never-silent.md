# 0003. Rule conflicts are recorded with explicit precedence, never silently merged

## Status

Accepted

## Context

A `ReviewPolicy` can be assembled from several Review Packs plus the repository's own criteria (`RepositoryContext.criteria`). These can disagree: a stack pack's default rule may contradict what the repository's own `CONTRIBUTING.md` states, or two concern packs may pull in opposite directions.

"Never opine without evidence" applies one level up: a result that applied one rule and silently discarded a contradicting one is itself an unattributed opinion. ESLint's flat config resolves conflicts implicitly by order and keeps no record of what was overridden; OPA's bundle composition treats conflicts as something to catch and combine by an explicit strategy. Debuggatha's evidence-first identity fits the second posture.

## Decision

1. **Conflict detection is opt-in per rule, not semantic analysis.** A `Rule` may declare `contradicts: string[]`. Resolution flags a conflict when two selected rules name each other, or when a pack `Rule` and a repository `CriteriaRule` share the same `(category, appliesTo)` concern key. Detecting contradictions between prose statements is out of scope.
2. **A fixed precedence resolves every detected conflict.** The repository's own `CriteriaRule` beats any pack `Rule`; between two pack rules, the pack named earlier in `ReviewRequest.requestedPackIds` wins.
3. **The losing rule is never dropped from the record.** Each conflict becomes a `ConflictRecord { winningRuleId, losingRuleId, reason }` in `ReviewPolicy.conflicts`. The loser does not appear in `ReviewPolicy.rules`, so Skills cannot apply it, but it stays traceable from the policy.

## Consequences

- Anyone can answer "was there a rule that would have said something different here, and why didn't it apply" from `ReviewPolicy.conflicts`.
- Two rules that should conflict but were not cross-referenced coexist, and can produce contradictory findings with no flag. This is a known limitation.
- Precedence is fixed and global, not configurable per request.

## Alternatives considered

- **Silent last-writer-wins.** Rejected: it fits a config written and read by one person, not a system whose promise is that every finding traces to a source.
- **Refuse to assemble a policy when any conflict is detected.** Rejected: any two packs touching the same concern would make Debuggatha unusable.
- **Detect contradictions with a model.** Rejected: Rule Resolution must be deterministic and model-independent, and must produce the same `ReviewPolicy.id` for the same inputs (ADR-0005).
