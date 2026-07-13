import type { CriteriaProfile } from "@debuggatha/repository-intelligence";
import type { EngineeringConventionItem } from "../types.js";

/**
 * Engineering Conventions (Epic 14): retypes
 * `@debuggatha/repository-intelligence`'s already-parsed `CriteriaRule[]`
 * (Epic 1's Criteria Resolution — rule-shaped Markdown headings,
 * `.eslintrc`/`rustfmt.toml`/etc. config extraction) into this package's
 * structured-fact model. Never reparses a single file — "avoid
 * duplicate parsing across subsystems." Every item is `"documented"`:
 * `CriteriaProfile` only ever contains rules a human actually wrote down
 * somewhere (a heading, a config key), never a guess.
 */
export function buildConventionItems(criteria: CriteriaProfile): EngineeringConventionItem[] {
  return criteria.rules.map((rule) => ({
    id: `engineering-convention:${rule.id}`,
    category: "engineering-convention",
    confidence: "documented",
    evidence: [{ file: rule.source.file, detail: rule.source.detail }],
    source: rule.source.file,
    rule: rule.description,
  }));
}
