# 0003. Rule conflicts are recorded with explicit precedence, never silently merged

## Status

Accepted

## Context

A `ReviewPolicy` can be assembled from multiple sources at once: several
Review Packs plus the repository's own `RepositoryContext.criteria`
(`CriteriaRule[]`). These can disagree — a stack pack's default rule
might contradict something the repository's own `CONTRIBUTING.md`
states, or two concern packs (e.g. a Performance pack and an Accessibility
pack) might pull in opposite directions on the same concern.

CLAUDE.md's core design principle is "never opine without evidence" —
every `Finding` must cite a real source. Silently dropping a losing rule
during policy assembly would violate the spirit of that principle one
level up: a `ReviewResult` that applied one rule and silently discarded a
contradicting one is itself an unattributed opinion ("we decided X
without saying why or that Y was overruled").

Research on comparable systems (see the Architecture doc §2) showed two
different postures: ESLint's flat config resolves conflicts implicitly by
array order (later layer wins, no record kept of what was overridden);
OPA's bundle composition instead treats overlapping/conflicting
namespaces as something to catch and combine via an explicit, documented
strategy rather than quietly merge. Debuggatha's evidence-first identity
makes the OPA posture the correct fit, not ESLint's.

## Decision

1. **Conflict detection is opt-in per rule, not automatic semantic
   analysis.** A `Rule` may declare `contradicts: string[]` (other rule
   ids, potentially in other packs, it's known to conflict with). Rule
   Resolution only flags a conflict when two selected rules explicitly
   name each other this way, or when a Pack's `Rule` and a repo
   `CriteriaRule` share the same `(category, appliesTo)` "concern key" —
   see the Architecture doc §7 for the precise definition. General
   NLP-based contradiction detection between two prose statements is
   explicitly out of scope (Architecture doc §11).
2. **Fixed precedence order resolves every detected conflict:** the
   repository's own `CriteriaRule` beats any Pack `Rule`; between two
   Pack rules, the pack named earlier in
   `ReviewRequest.requestedPackIds` wins.
3. **The losing rule is never dropped from the record.** Every detected
   conflict becomes a `ConflictRecord { winningRuleId, losingRuleId,
   reason }` entry in `ReviewPolicy.conflicts`. The losing rule does not
   appear in `ReviewPolicy.rules` (so Skills don't accidentally apply
   it), but it is fully traceable from the policy object.

## Consequences

- A `ReviewResult` reviewer (human or tool) can always answer "was there
  a rule that would have said something different here, and why didn't
  it apply" by inspecting `ReviewPolicy.conflicts` — extends "always
  inspectable, always citable" (CLAUDE.md's definition of a Review
  Policy) to the conflicts themselves, not just the winning rules.
- Conflict detection has a real gap: two rules that *should* conflict but
  weren't explicitly cross-referenced by their authors will silently
  coexist, possibly producing contradictory Findings in the same
  `ReviewResult` with no flag raised. This is an accepted v1 limitation
  (Architecture doc §12), not a hidden one.
- Precedence is fixed and global, not configurable per review request in
  v1 — a repository cannot currently say "for this run, prefer the OWASP
  pack over my own CONTRIBUTING.md." If that need surfaces, it is a
  natural extension of `ReviewRequest`, not a change to this ADR's
  decision about *how* conflicts get recorded.

## Alternatives considered

- **Silent last-writer-wins (ESLint's flat-config posture).** Rejected:
  fits a config authored and read by the same human, not a system whose
  central promise is "every finding traces back to a real source."
- **Refuse to assemble a policy at all when any conflict is detected**
  (fail closed on conflicts, not just on missing dependencies). Rejected:
  would make Debuggatha unusable the moment any two packs a repo wants to
  combine (e.g. a stack pack + a concern pack) happen to touch the same
  concern — too strict for v1's stated goal of validating the core
  hypothesis with 2–3 packs.
- **Automatic contradiction detection via an LLM call during policy
  assembly.** Rejected: Rule Resolution must stay deterministic and
  model-independent (the same requirement CLAUDE.md places on Core
  Skills) — policy assembly runs before any model is invoked and must
  produce the same `ReviewPolicy.id` for the same inputs every time (see
  ADR-0005). An LLM-based conflict check would break that determinism.
