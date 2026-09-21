export type { CacheEntry, Fingerprint, RepositoryContextCache } from "./cache/index.js";
export { createInMemoryCache } from "./cache/index.js";
export { deriveCapabilities } from "./capabilities.js";
export type { BuildRepositoryContextOptions } from "./snapshot.js";
export { buildRepositoryContext } from "./snapshot.js";
export { summarizeCapabilities } from "./summary.js";
export type {
  Capability,
  CapabilityConfidence,
  CapabilityKind,
  CapabilityOrigin,
  CriteriaProfile,
  CriteriaRule,
  DependencyProfile,
  DocumentationKind,
  DocumentationProfile,
  DocumentationSource,
  Evidence,
  OpenQuestion,
  PlatformTarget,
  RepositoryContext,
  StackProfile,
  StackSignal,
  UnderstandingConfidence,
  UnderstandingProfile,
  WorkspaceType,
} from "./types.js";
