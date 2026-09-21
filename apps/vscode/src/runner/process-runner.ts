import { type ChildProcess, fork, spawn } from "node:child_process";
import { executeJob } from "./execute.js";
import {
  ReviewCancelled,
  type ReviewJob,
  type ReviewJobResult,
  type ReviewRunner,
  type WorkerMessage,
} from "./protocol.js";

/** Stops a review and everything it started: an analyzer is a child of the review, not of us. */
export function killTree(child: ChildProcess): void {
  const pid = child.pid;
  if (pid === undefined) return;
  try {
    if (process.platform === "win32") {
      spawn("taskkill", ["/pid", String(pid), "/T", "/F"], { stdio: "ignore" });
    } else {
      // The review leads its own process group (`detached`), so the whole group can be signalled.
      process.kill(-pid, "SIGKILL");
    }
  } catch {
    child.kill("SIGKILL");
  }
}

const STDERR_LIMIT = 2_000;

/** Runs each review in its own process, so it can be cancelled and cannot freeze the editor. */
export function createProcessRunner(workerPath: string): ReviewRunner {
  return {
    run(job: ReviewJob, signal: AbortSignal): Promise<ReviewJobResult> {
      return new Promise((resolve, reject) => {
        if (signal.aborted) {
          reject(new ReviewCancelled());
          return;
        }
        const child = fork(workerPath, [], {
          detached: process.platform !== "win32",
          silent: true,
          // Inside the extension host `process.execPath` is Electron, which runs as Node only with this.
          env: { ...process.env, ELECTRON_RUN_AS_NODE: "1" },
        });

        let settled = false;
        let stderr = "";
        const settle = (finish: () => void) => {
          if (settled) return;
          settled = true;
          signal.removeEventListener("abort", onAbort);
          finish();
        };
        const onAbort = () => {
          killTree(child);
          settle(() => reject(new ReviewCancelled()));
        };
        signal.addEventListener("abort", onAbort, { once: true });

        child.stderr?.on("data", (chunk: Buffer) => {
          stderr = (stderr + chunk.toString()).slice(-STDERR_LIMIT);
        });
        child.on("message", (message: WorkerMessage) => {
          settle(() =>
            message.type === "done" ? resolve(message.result) : reject(new Error(message.message)),
          );
        });
        child.on("error", (error) => settle(() => reject(error)));
        child.on("exit", (code) => {
          settle(() =>
            reject(
              new Error(
                `The review process ended unexpectedly (code ${code})${stderr.trim() ? `: ${stderr.trim()}` : "."}`,
              ),
            ),
          );
        });
        child.send({ job });
      });
    },
  };
}

/** The same work in this process: for tests, and as the fallback when the worker file is missing. */
export function createInProcessRunner(): ReviewRunner {
  return {
    async run(job, signal) {
      if (signal.aborted) throw new ReviewCancelled();
      const result = await executeJob(job);
      if (signal.aborted) throw new ReviewCancelled();
      return result;
    },
  };
}
