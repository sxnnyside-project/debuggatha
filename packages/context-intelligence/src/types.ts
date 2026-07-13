import type { DocumentationKind } from "@debuggatha/repository-intelligence";

/**
 * Context Intelligence domain model (Epic 14).
 *
 * The load-bearing decision: this package extracts *facts*, never
 * interpretations. A `ContextItem` never stores prose like "the project
 * favors small functions" — it stores a structured claim
 * (`{ rule: "Prefer small functions" }`) with its own evidence and
 * confidence. What that claim *means* for a specific finding is a Review
 * Skill's or the Runtime Engine's job, not this package's — this
 * package's responsibility ends at producing structured context, never
 * at judging code against it (see CLAUDE.md/epic "It does not produce
 * findings. It produces understanding.").
 */

/**
 * Never conflated. `"documented"` means a human wrote this down somewhere
 * (a CONTRIBUTING.md heading, a CODEOWNERS entry) — the strongest claim
 * this package makes. `"detected"` means a deterministic scan found
 * concrete evidence (a config file's presence, a manifest field) without
 * anyone having to say so in prose. `"inferred"` is the weakest: derived
 * from indirect signals (e.g. a capability detected only from a
 * dependency, not a dedicated config file). Never promoted upward — a
 * `"detected"` fact never becomes `"documented"` just because it seems
 * likely.
 */
export type ContextConfidence = "documented" | "detected" | "inferred";

export interface ContextEvidence {
  file: string;
  detail: string;
}

export type ContextCategory =
  | "engineering-convention"
  | "documentation"
  | "capability"
  | "workflow"
  | "ownership"
  | "project-metadata"
  | "git";

interface ContextItemBase {
  id: string;
  confidence: ContextConfidence;
  evidence: ContextEvidence[];
  /** The file/tool this item was extracted from — every item is explainable back to something concrete. */
  source: string;
}

/** A repository-authored engineering rule — e.g. from a rule-shaped CONTRIBUTING.md heading. Mirrors `@debuggatha/repository-intelligence`'s `CriteriaRule`, retyped into this package's category taxonomy rather than reparsed. */
export interface EngineeringConventionItem extends ContextItemBase {
  category: "engineering-convention";
  rule: string;
}

/** One documentation artifact's existence and structural role — not its prose content (see package README "extract engineering intent rather than prose"). */
export interface DocumentationContextItem extends ContextItemBase {
  category: "documentation";
  kind: DocumentationKind;
  title: string | undefined;
}

/** Wraps a `Capability` (`@debuggatha/repository-intelligence`, Epic 12.5) in this package's category taxonomy — never reparsed, never re-detected. */
export interface CapabilityContextItem extends ContextItemBase {
  category: "capability";
  capabilityId: string;
  capabilityKind: string;
}

export type WorkflowKind = "ci" | "release" | "branching-convention" | "commit-convention";

export interface WorkflowContextItem extends ContextItemBase {
  category: "workflow";
  workflowKind: WorkflowKind;
  detail: string;
}

/** Wraps an `OwnershipSignal` (`@debuggatha/analysis-engine`, Epic 12) — only ever emitted when that signal's own `source` isn't `"unknown"` (never a fabricated owner). */
export interface OwnershipContextItem extends ContextItemBase {
  category: "ownership";
  moduleId: string;
  owners: string[];
}

export interface ProjectMetadataItem extends ContextItemBase {
  category: "project-metadata";
  key: string;
  value: string;
}

export type GitContextKey = "default-branch" | "active-branch";

export interface GitContextItem extends ContextItemBase {
  category: "git";
  key: GitContextKey;
  value: string;
}

export type ContextItem =
  | EngineeringConventionItem
  | DocumentationContextItem
  | CapabilityContextItem
  | WorkflowContextItem
  | OwnershipContextItem
  | ProjectMetadataItem
  | GitContextItem;

export interface ContextIntelligenceResult {
  rootDir: string;
  generatedAt: string;
  items: ContextItem[];
}
