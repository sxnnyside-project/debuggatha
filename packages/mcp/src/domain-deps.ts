import * as core from "@debuggatha/engine";

/**
 * Every domain function a tool handler is allowed to call, bundled as one
 * injectable object. Real handlers use `createDefaultDomainDeps()` (the
 * real `@debuggatha/core` façade); tests substitute a fake bundle — "mock
 * the domain, validate the adapter" (epic "Testing").
 */
export interface DomainDeps {
  buildRepositoryContext: typeof core.buildRepositoryContext;
  createReviewRequest: typeof core.createReviewRequest;
  assemblePolicy: typeof core.assemblePolicy;
  createReviewSession: typeof core.createReviewSession;
  transitionSession: typeof core.transitionSession;
  createReviewResult: typeof core.createReviewResult;
  reviewDiff: typeof core.reviewDiff;
  reviewArchitecture: typeof core.reviewArchitecture;
  reviewFiles: typeof core.reviewFiles;
  synchronizeReviewResult: typeof core.synchronizeReviewResult;
  loadLedger: typeof core.loadLedger;
  saveLedger: typeof core.saveLedger;
  listEntries: typeof core.listEntries;
  getEntry: typeof core.getEntry;
  updateFindingStatus: typeof core.updateFindingStatus;
  summarizeLedger: typeof core.summarizeLedger;
  summarizeCapabilities: typeof core.summarizeCapabilities;
  loadMemoryStore: typeof core.loadMemoryStore;
  filterSuppressedFindings: typeof core.filterSuppressedFindings;
}

export function createDefaultDomainDeps(): DomainDeps {
  return {
    buildRepositoryContext: core.buildRepositoryContext,
    createReviewRequest: core.createReviewRequest,
    assemblePolicy: core.assemblePolicy,
    createReviewSession: core.createReviewSession,
    transitionSession: core.transitionSession,
    createReviewResult: core.createReviewResult,
    reviewDiff: core.reviewDiff,
    reviewArchitecture: core.reviewArchitecture,
    reviewFiles: core.reviewFiles,
    synchronizeReviewResult: core.synchronizeReviewResult,
    loadLedger: core.loadLedger,
    saveLedger: core.saveLedger,
    listEntries: core.listEntries,
    getEntry: core.getEntry,
    updateFindingStatus: core.updateFindingStatus,
    summarizeLedger: core.summarizeLedger,
    summarizeCapabilities: core.summarizeCapabilities,
    loadMemoryStore: core.loadMemoryStore,
    filterSuppressedFindings: core.filterSuppressedFindings,
  };
}
