/**
 * Runs one review at a time and lets the person cancel it.
 *
 * @packageDocumentation
 */

import type { ReviewJobResult } from '../contracts/index.js';
import type { ReviewJob } from '../daemon/api.js';
import type { Progress } from '../daemon/client.js';
import { DaemonUnavailable } from '../daemon/connection.js';

/** Whatever runs a review: the daemon. Rejects when `signal` aborts, after the work has stopped. */
export interface ReviewRunner {
  /** The review that ran. */
  review(
    job: ReviewJob,
    signal: AbortSignal,
    onProgress?: (progress: Progress) => void,
  ): Promise<ReviewJobResult>;
}

/** Who asked: a person (a command) or the editor (a save). */
export type Origin = 'manual' | 'save';

/** How a review ended: with a result, failed, or cancelled. */
export type Outcome =
  | { status: 'done'; result: ReviewJobResult }
  | { status: 'cancelled' }
  /** `unavailable`: the daemon could not be started; `setting`: a setting fixes it. */
  | { status: 'failed'; error: string; unavailable?: boolean; setting?: boolean }
  /** Nothing to do: a save arrived during a person's review, or the file is not the project's own. */
  | { status: 'skipped' };

/**
 * One review at a time. A person's review replaces whatever is running (a review-on-save that
 * is about to be stale, or an older manual one), and a save never interrupts a person's.
 */
export class ReviewCoordinator {
  private current:
    | { origin: Origin; controller: AbortController; settled: Promise<unknown> }
    | undefined;

  constructor(
    private readonly runner: ReviewRunner,
    private readonly onChange: (running: Origin | undefined) => void = () => {},
  ) {}

  /** What started the review that is running, or `undefined` when none is. */
  get running(): Origin | undefined {
    return this.current?.origin;
  }

  /**
   * Cancels the review that is running; the daemon stops its work and the analyzers it started.
   */
  cancel(): void {
    this.current?.controller.abort();
  }

  /** Runs `job`. A person's review replaces the one running; a save never interrupts a person's review and is skipped. */
  async run(
    origin: Origin,
    job: ReviewJob,
    onProgress?: (progress: Progress) => void,
  ): Promise<Outcome> {
    if (this.current) {
      if (origin === 'save' && this.current.origin === 'manual') return { status: 'skipped' };
      this.current.controller.abort();
      await this.current.settled;
    }

    const controller = new AbortController();
    const work = this.runner.review(job, controller.signal, onProgress).then(
      (result): Outcome => ({ status: 'done', result }),
      (error: unknown): Outcome =>
        controller.signal.aborted || (error as { code?: string }).code === 'cancelled'
          ? { status: 'cancelled' }
          : (error as { code?: string }).code === 'nothing_to_review'
            ? { status: 'skipped' }
            : error instanceof DaemonUnavailable
              ? {
                  status: 'failed',
                  error: error.message,
                  unavailable: true,
                  setting: error.setting,
                }
              : { status: 'failed', error: error instanceof Error ? error.message : String(error) },
    );
    const mine = { origin, controller, settled: work };
    this.current = mine;
    this.onChange(origin);
    try {
      return await work;
    } finally {
      if (this.current === mine) {
        this.current = undefined;
        this.onChange(undefined);
      }
    }
  }
}
