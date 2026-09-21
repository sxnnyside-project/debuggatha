import { createHash } from "node:crypto";

/**
 * Bumped whenever `Rule`/`ReviewPack`/`ReviewPolicy`'s shape changes in a
 * way that would change resolution output for the same inputs — guards
 * against silently reusing an old `ReviewPolicy.id` under a changed
 * resolution algorithm (ADR-0005).
 */
export const KNOWLEDGE_SCHEMA_VERSION = "1";

/**
 * Deterministic JSON serialization: object keys sorted recursively so two
 * structurally-equal values always serialize identically regardless of
 * property insertion order (ADR-0005: "a naive `Object.keys()` iteration
 * order bug would silently break reproducibility").
 */
export function canonicalStringify(value: unknown): string {
  return JSON.stringify(sortForCanonicalForm(value));
}

function sortForCanonicalForm(value: unknown): unknown {
  if (Array.isArray(value)) {
    return value.map(sortForCanonicalForm);
  }
  if (value !== null && typeof value === "object") {
    const source = value as Record<string, unknown>;
    const sorted: Record<string, unknown> = {};
    for (const key of Object.keys(source).sort()) {
      sorted[key] = sortForCanonicalForm(source[key]);
    }
    return sorted;
  }
  return value;
}

export function sha256Hex(input: string): string {
  return createHash("sha256").update(input).digest("hex");
}
