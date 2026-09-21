/**
 * The review domain: what Debuggatha knows and the rules it plays by.
 * No I/O adapters, no orchestration, no packs' content — `@debuggatha/engine`
 * runs reviews on top of this and `@debuggatha/packs` supplies the rules.
 *
 * Dependency direction inside this package, lowest first:
 * repository-intelligence → review-engine → knowledge-system →
 * findings-ledger → repository-memory.
 */
export * from "./findings-ledger/index.js";
// Names two modules both export. Explicit exports win over `export *`, so each
// ambiguity is settled here: the review domain keeps the plain name.
export { CURRENT_SCHEMA_VERSION } from "./findings-ledger/index.js";
export * from "./knowledge-system/index.js";
export type { Evidence as RepositoryEvidence } from "./repository-intelligence/index.js";
export * from "./repository-intelligence/index.js";
export * from "./repository-memory/index.js";
export { CURRENT_SCHEMA_VERSION as MEMORY_SCHEMA_VERSION } from "./repository-memory/index.js";
export type { Evidence } from "./review-engine/index.js";
export * from "./review-engine/index.js";
