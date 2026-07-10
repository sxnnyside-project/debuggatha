/**
 * Deliberately minimal stand-ins. The Review Engine is independent from
 * `@debuggatha/review-packs` and `@debuggatha/policies` (out of scope for
 * this epic — see CLAUDE.md) — it references a pack or policy by id only,
 * never their concrete content or logic.
 *
 * `ReviewPackReference.version` is a real field (not folded into `id` as
 * a `"packId@version"` string) — Review Packs are independently authored
 * and versioned (see docs/adr/0002), and a Finding citing a pack needs an
 * unambiguous, structured pin. `ReviewPolicyReference` has no separate
 * version: a policy is identified entirely by its own deterministic id
 * (see docs/adr/0005), not by an authored version number.
 */
export interface ReviewPackReference {
  id: string;
  version: string;
}

export interface ReviewPolicyReference {
  id: string;
}
