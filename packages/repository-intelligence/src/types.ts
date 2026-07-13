/**
 * Repository Intelligence domain model.
 *
 * This package is intentionally independent from MCP, VS Code, the CLI,
 * LLM providers, and the Review Engine (Findings, Review Packs, Policies —
 * see CLAUDE.md). Other modules depend only on `RepositoryContext`; none
 * of the types here import from `@debuggatha/shared` on purpose, so this
 * package never has to know what a "Finding" or a "Review Policy" is.
 */

/** A single, citable justification for a claim — never opine without evidence. */
export interface Evidence {
  file: string;
  detail: string;
}

/** A detected fact (a language, a framework, a build tool...) with its evidence trail. */
export interface StackSignal {
  value: string;
  evidence: Evidence[];
}

export type WorkspaceType = "single-package" | "monorepo";
export type PlatformTarget = "web" | "desktop" | "mobile";

export interface StackProfile {
  languages: StackSignal[];
  frameworks: StackSignal[];
  buildSystems: StackSignal[];
  packageManagers: StackSignal[];
  runtimes: StackSignal[];
  platformTargets: PlatformTarget[];
  workspaceType: WorkspaceType;
  workspaceEvidence: Evidence[];
  manifests: string[];
}

/**
 * Repository Capability Resolution (Epic 12.5). Additive, never
 * mutually exclusive — a repository can carry `"typescript"`,
 * `"javascript"`, `"react"`, `"bun"`, `"turborepo"`, and `"tauri"`
 * capabilities simultaneously; nothing here collapses them into one
 * "stack" label. This is the canonical, machine-matchable contract Review
 * Pack resolution/Review Policy assembly consume — `StackProfile` above
 * remains the display-oriented, human-readable representation (kept for
 * backwards compatibility — nothing about it changed), generated from
 * these same underlying signals, not the other way around.
 */
export type CapabilityKind = "language" | "framework" | "platform" | "tooling" | "characteristic";
export type CapabilityConfidence = "high" | "medium" | "low";
export type CapabilityOrigin = "manifest" | "lockfile" | "config" | "convention";

export interface Capability {
  /** Normalized (lowercase, alphanumeric-only) — e.g. "typescript", "react", "aspnetcore". */
  id: string;
  kind: CapabilityKind;
  confidence: CapabilityConfidence;
  evidence: Evidence[];
  origin: CapabilityOrigin;
}

export interface DependencyProfile {
  lockfiles: Evidence[];
  declaredVersions: Record<string, string>;
  runtimeConstraints: Record<string, string>;
}

export type DocumentationKind =
  | "readme"
  | "claude-memory"
  | "contributing"
  | "architecture"
  | "adr"
  | "prd"
  | "roadmap"
  | "other";

export interface DocumentationSource {
  file: string;
  kind: DocumentationKind;
  title: string | undefined;
  wordCount: number;
}

export interface DocumentationProfile {
  sources: DocumentationSource[];
  summary: string;
}

export interface CriteriaRule {
  id: string;
  description: string;
  source: Evidence;
}

export interface CriteriaProfile {
  rules: CriteriaRule[];
  sources: string[];
}

export type UnderstandingConfidence = "high" | "medium" | "low";

export interface OpenQuestion {
  question: string;
  reason: string;
}

export interface UnderstandingProfile {
  confidence: UnderstandingConfidence;
  openQuestions: OpenQuestion[];
}

/**
 * The normalized, immutable snapshot of a repository. Produced once per
 * scan (or served from cache — see `RepositoryContextCache`) and frozen
 * with `Object.freeze` so nothing downstream can mutate shared state.
 */
export interface RepositoryContext {
  readonly rootDir: string;
  readonly generatedAt: string;
  readonly hasGit: boolean;
  readonly stack: StackProfile;
  readonly capabilities: readonly Capability[];
  readonly dependencies: DependencyProfile;
  readonly documentation: DocumentationProfile;
  readonly criteria: CriteriaProfile;
  readonly understanding: UnderstandingProfile;
}
