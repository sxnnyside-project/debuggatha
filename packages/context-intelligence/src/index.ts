export {
  buildContextFingerprint,
  type ContextFingerprint,
  type ContextIntelligenceCache,
  type ContextIntelligenceCacheEntry,
  createInMemoryContextCache,
  hasContextFingerprintChanged,
} from "./cache.js";
export { type BuildContextIntelligenceOptions, buildContextIntelligence } from "./pipeline.js";
export { conventionsToPolicyStatements } from "./policy-statements.js";
export { buildCapabilityItems } from "./sources/capabilities.js";
export { buildConventionItems } from "./sources/conventions.js";
export { buildDocumentationItems } from "./sources/documentation.js";
export { buildGitContextItems } from "./sources/git.js";
export { buildOwnershipItems } from "./sources/ownership.js";
export { buildProjectMetadataItems } from "./sources/project-metadata.js";
export { detectWorkflow } from "./sources/workflow.js";
export type {
  CapabilityContextItem,
  ContextCategory,
  ContextConfidence,
  ContextEvidence,
  ContextIntelligenceResult,
  ContextItem,
  DocumentationContextItem,
  EngineeringConventionItem,
  GitContextItem,
  GitContextKey,
  OwnershipContextItem,
  ProjectMetadataItem,
  WorkflowContextItem,
  WorkflowKind,
} from "./types.js";
