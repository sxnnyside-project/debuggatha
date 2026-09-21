import {
  ReviewCancelled,
  type ReviewJob,
  type ReviewJobResult,
  type ReviewRunner,
} from "../runner/protocol.js";

/** Who asked: a person (a command) or the editor (a save). */
export type Origin = "manual" | "save";

export type Outcome =
  | { status: "done"; result: ReviewJobResult }
  | { status: "cancelled" }
  | { status: "failed"; error: string }
  /** A save arrived while a person's review was running; theirs is not interrupted for it. */
  | { status: "skipped" };

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

  get running(): Origin | undefined {
    return this.current?.origin;
  }

  cancel(): void {
    this.current?.controller.abort();
  }

  async run(origin: Origin, job: ReviewJob): Promise<Outcome> {
    if (this.current) {
      if (origin === "save" && this.current.origin === "manual") return { status: "skipped" };
      this.current.controller.abort();
      await this.current.settled;
    }

    const controller = new AbortController();
    const work = this.runner.run(job, controller.signal).then(
      (result): Outcome => ({ status: "done", result }),
      (error: unknown): Outcome =>
        error instanceof ReviewCancelled || controller.signal.aborted
          ? { status: "cancelled" }
          : { status: "failed", error: error instanceof Error ? error.message : String(error) },
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
