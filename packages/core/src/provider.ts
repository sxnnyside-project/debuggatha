/**
 * The RepositoryProvider establishes an architectural boundary separating
 * repository operations from transport adapters (like MCP or CLI).
 *
 * Adapters should no longer depend directly on Git implementations; instead,
 * they should inject a RepositoryProvider into the domain or use it locally
 * to abstract away the underlying version control system.
 */
export interface RepositoryProvider {
  /** Gets the absolute path to the root of the repository. */
  getRootDir(): string;

  /** Gets the unified diff of the repository. If `base` is provided, diffs against it. */
  getDiff(base?: string): Promise<string | undefined>;

  /** Gets the current status of the repository (e.g., modified files). */
  getStatus(): Promise<string>;

  /** Gets the name of the current branch. */
  getBranch(): Promise<string>;
}
