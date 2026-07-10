/**
 * `@debuggatha/core` is the stable façade MCP, the CLI, and the VS Code
 * panel depend on — see the root README's package graph. It re-exports
 * each epic's package as it lands: Repository Intelligence (Epic 1), the
 * Review Engine domain (Epic 2), the Knowledge System (Epic 3), the
 * Findings Ledger (Epic 4), plus the thin `@debuggatha/policies` wiring
 * and `@debuggatha/skills`' Review Skill functions — so Epic 5's MCP
 * package can depend on this one façade instead of five separate domain
 * packages. This is exports only; the multi-step orchestration (build a
 * RepositoryContext, assemble a policy, run a Skill, sync the result into
 * a Ledger) is composed by whatever calls these functions — today that's
 * `@debuggatha/mcp`'s tool handlers, not this package.
 */

export type {
  FindingFingerprint,
  FindingLifecycleStatus,
  FindingMatcher,
  HistoryEvent,
  Ledger,
  LedgerEntry,
  LedgerEntryFilter,
  LedgerSummary,
  MatchOutcome,
  RepositorySnapshotRef,
  ReviewAssociation,
  SyncReport,
  SyncScope,
  TransitionOrigin,
} from "@debuggatha/findings-ledger";
export {
  associationFor,
  CURRENT_SCHEMA_VERSION,
  canTransitionFinding,
  computeFindingFingerprint,
  createEmptyLedger,
  createLedgerEntry,
  defaultFindingMatcher,
  deserializeLedger,
  FINDING_LIFECYCLE_STATUSES,
  getEntry,
  getHistory,
  isActiveFindingStatus,
  ledgerFilePath,
  listEntries,
  loadLedger,
  refreshLedgerEntry,
  saveLedger,
  serializeLedger,
  snapshotRefFor,
  summarizeLedger,
  synchronizeReviewResult,
  transitionFinding,
  updateFindingStatus,
} from "@debuggatha/findings-ledger";
export type {
  CapabilityRegistry,
  ConflictRecord,
  KnowledgeEntry,
  PackDependency,
  PolicyRule,
  PolicyRuleOrigin,
  ReviewPack,
  ReviewPackKind,
  ReviewPolicy,
  Rule,
  RuleScope,
  SkillContext,
  SkillDescriptor,
  SkillTier,
} from "@debuggatha/knowledge-system";
export {
  assembleContext,
  createInMemoryRegistry,
  resolvePolicy,
  SKILL_TIERS,
  validateReviewPack,
} from "@debuggatha/knowledge-system";
export { assemblePolicy, defaultCapabilityRegistry } from "@debuggatha/policies";
export type {
  BuildRepositoryContextOptions,
  CriteriaProfile,
  CriteriaRule,
  DependencyProfile,
  DocumentationProfile,
  Evidence as RepositoryEvidence,
  RepositoryContext,
  RepositoryContextCache,
  StackProfile,
  StackSignal,
  UnderstandingProfile,
} from "@debuggatha/repository-intelligence";
export {
  buildRepositoryContext,
  createInMemoryCache,
} from "@debuggatha/repository-intelligence";
export type {
  Category,
  Confidence,
  CreateFindingInput,
  Evidence,
  Finding,
  FindingLocation,
  Recommendation,
  ReviewDepth,
  ReviewLifecycleStatus,
  ReviewPackReference,
  ReviewPolicyReference,
  ReviewRequest,
  ReviewResult,
  ReviewScope,
  ReviewSession,
  ReviewSource,
  ReviewSummary,
  Severity,
} from "@debuggatha/review-engine";
export {
  CATEGORIES,
  canTransition,
  createFinding,
  createReviewRequest,
  createReviewResult,
  createReviewSession,
  isTerminal,
  REVIEW_DEPTHS,
  REVIEW_LIFECYCLE_STATUSES,
  summarizeFindings,
  transitionSession,
} from "@debuggatha/review-engine";
export { reviewArchitecture, reviewDiff, reviewFiles } from "@debuggatha/skills";
export type { RepositoryProvider } from "./provider.js";
