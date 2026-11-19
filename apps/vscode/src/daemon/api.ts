import type {
  LedgerEntry,
  LifecycleStatus,
  RepositoryContext,
  ReviewJobResult,
  ReviewPolicy,
} from '../contracts/index.js';
import type { Progress } from './client.js';
import type { DaemonConnection } from './connection.js';

/** What a review covers. Paths are relative to the repository, with forward slashes. */
export type ReviewScope =
  | { kind: 'workspace' }
  /** `owned`: leave out what the project does not own (dependencies, build output). */
  | { kind: 'files'; paths: string[]; owned?: boolean }
  | { kind: 'diff'; diff?: string; base?: string };

export interface ReviewJob {
  scope: ReviewScope;
  packIds: string[];
  /** `off` runs the built-in detectors only. Which analyzers may run is the person's, at start. */
  analyzers: { mode: 'off' | 'auto' };
  /** Also have the model the person configured read the changed code. */
  semantic: boolean;
}

const PAGE = 500;

/** The typed calls of the extension. Each is one Protocol v1 method; nothing else is decided here. */
export class Daemon {
  constructor(private readonly connection: DaemonConnection) {}

  async review(
    job: ReviewJob,
    signal: AbortSignal,
    onProgress?: (progress: Progress) => void,
  ): Promise<ReviewJobResult> {
    const result = await this.connection.request<ReviewJobResult | { status: 'input_required' }>(
      'review/run',
      { ...job, limit: 1 },
      { signal, ...(onProgress ? { onProgress } : {}) },
    );
    if ('status' in result)
      throw new Error('The review asked for a model this extension cannot give.');
    return result;
  }

  /** The whole ledger, in pages. */
  async ledger(): Promise<LedgerEntry[]> {
    const entries: LedgerEntry[] = [];
    let cursor: string | null = null;
    do {
      const page: { entries: LedgerEntry[]; nextCursor: string | null } =
        await this.connection.request('findings/list', {
          limit: PAGE,
          ...(cursor ? { cursor } : {}),
        });
      entries.push(...page.entries);
      cursor = page.nextCursor;
    } while (cursor);
    return entries;
  }

  async setStatus(id: string, status: LifecycleStatus): Promise<LedgerEntry> {
    const { entry } = await this.connection.request<{ entry: LedgerEntry }>('findings/update', {
      id,
      status,
    });
    return entry;
  }

  /** The comment that accepts a finding in the code, with a placeholder where the reason goes. */
  async inlineComment(id: string): Promise<string> {
    const { ifYouAcceptIt } = await this.connection.request<{
      ifYouAcceptIt: { inlineComment: string };
    }>('findings/explain', { id });
    return ifYouAcceptIt.inlineComment;
  }

  async context(): Promise<RepositoryContext> {
    const { context } = await this.connection.request<{ context: RepositoryContext }>(
      'repository/context',
    );
    return context;
  }

  async policy(packIds: string[]): Promise<ReviewPolicy> {
    const { policy } = await this.connection.request<{ policy: ReviewPolicy }>(
      'repository/policy',
      {
        packIds,
      },
    );
    return policy;
  }
}
