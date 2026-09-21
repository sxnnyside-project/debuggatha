import { executeJob } from "./execute.js";
import type { ReviewJob, WorkerMessage } from "./protocol.js";

/**
 * The process a review runs in (`dist/review-worker.js`). Analyzers are run synchronously, and
 * one can take minutes: doing that in the extension host would freeze every extension, and a
 * cancelled review could not stop it. A separate process can be killed.
 */
function reply(message: WorkerMessage): void {
  process.send?.(message, () => process.exit(message.type === "done" ? 0 : 1));
}

process.once("message", async (message: { job: ReviewJob }) => {
  try {
    reply({ type: "done", result: await executeJob(message.job) });
  } catch (error) {
    reply({ type: "error", message: error instanceof Error ? error.message : String(error) });
  }
});
