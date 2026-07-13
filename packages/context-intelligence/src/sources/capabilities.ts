import type { Capability } from "@debuggatha/repository-intelligence";
import type { CapabilityContextItem, ContextConfidence } from "../types.js";

/**
 * Maps a `Capability.confidence` (`"high"`/`"medium"`/`"low"` — Epic
 * 12.5's own scale) onto this package's `documented`/`detected`/
 * `inferred` scale. A capability is never `"documented"`: nobody writes
 * "this repo uses TypeScript" in prose for Repository Intelligence to
 * read — it's always machine-detected from a manifest/config/lockfile,
 * at varying strength.
 */
function toContextConfidence(confidence: Capability["confidence"]): ContextConfidence {
  return confidence === "high" ? "detected" : "inferred";
}

/** Wraps each `Capability` (`@debuggatha/repository-intelligence`, Epic 12.5) into this package's taxonomy — never re-detects a single one. */
export function buildCapabilityItems(capabilities: readonly Capability[]): CapabilityContextItem[] {
  return capabilities.map((capability) => ({
    id: `capability:${capability.kind}:${capability.id}`,
    category: "capability",
    confidence: toContextConfidence(capability.confidence),
    evidence: capability.evidence,
    source: capability.evidence[0]?.file ?? "repository",
    capabilityId: capability.id,
    capabilityKind: capability.kind,
  }));
}
