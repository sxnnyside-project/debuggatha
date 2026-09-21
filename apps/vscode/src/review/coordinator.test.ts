import { describe, expect, it } from "vitest";
import {
  ReviewCancelled,
  type ReviewJob,
  type ReviewJobResult,
  type ReviewRunner,
} from "../runner/protocol.js";
import { ReviewCoordinator } from "./coordinator.js";

const job: ReviewJob = {
  rootDir: "/w",
  scope: { kind: "workspace" },
  packIds: [],
  analyzers: { mode: "off" },
};
const result = (findings: number): ReviewJobResult => ({
  findings,
  analyzers: [],
  semantic: null,
  changes: { introduced: 0, fixed: 0, reopened: 0 },
  durationMs: 1,
});

/** A runner whose reviews finish when the test says so. */
function controlled() {
  const started: {
    resolve: (r: ReviewJobResult) => void;
    reject: (e: Error) => void;
    signal: AbortSignal;
  }[] = [];
  const runner: ReviewRunner = {
    run: (_job, signal) =>
      new Promise((resolve, reject) => {
        const entry = { resolve, reject, signal };
        started.push(entry);
        signal.addEventListener("abort", () => reject(new ReviewCancelled()), { once: true });
      }),
  };
  return { runner, started };
}

describe("ReviewCoordinator", () => {
  it("runs a review and reports its result, and tells when it starts and stops", async () => {
    const { runner, started } = controlled();
    const changes: (string | undefined)[] = [];
    const coordinator = new ReviewCoordinator(runner, (running) => changes.push(running));

    const outcome = coordinator.run("manual", job);
    expect(coordinator.running).toBe("manual");
    started[0]?.resolve(result(2));

    expect(await outcome).toEqual({ status: "done", result: result(2) });
    expect(coordinator.running).toBeUndefined();
    expect(changes).toEqual(["manual", undefined]);
  });

  it("cancel stops the running review", async () => {
    const { runner, started } = controlled();
    const coordinator = new ReviewCoordinator(runner);
    const outcome = coordinator.run("manual", job);
    coordinator.cancel();
    expect(await outcome).toEqual({ status: "cancelled" });
    expect(started[0]?.signal.aborted).toBe(true);
  });

  it("a person's review replaces one the editor started on save", async () => {
    const { runner, started } = controlled();
    const coordinator = new ReviewCoordinator(runner);
    const bySave = coordinator.run("save", job);
    const byPerson = coordinator.run("manual", job);

    expect(await bySave).toEqual({ status: "cancelled" });
    await Promise.resolve();
    started[1]?.resolve(result(4));
    expect(await byPerson).toEqual({ status: "done", result: result(4) });
  });

  it("a save never interrupts a person's review, and does not queue behind it", async () => {
    const { runner, started } = controlled();
    const coordinator = new ReviewCoordinator(runner);
    const byPerson = coordinator.run("manual", job);
    expect(await coordinator.run("save", job)).toEqual({ status: "skipped" });
    expect(started).toHaveLength(1);
    started[0]?.resolve(result(1));
    expect((await byPerson).status).toBe("done");
  });

  it("a newer save replaces an older one", async () => {
    const { runner, started } = controlled();
    const coordinator = new ReviewCoordinator(runner);
    const first = coordinator.run("save", job);
    const second = coordinator.run("save", job);
    expect(await first).toEqual({ status: "cancelled" });
    await Promise.resolve();
    started[1]?.resolve(result(0));
    expect((await second).status).toBe("done");
  });

  it("reports a review that fails, and can run again afterwards", async () => {
    const { runner, started } = controlled();
    const coordinator = new ReviewCoordinator(runner);
    const failing = coordinator.run("manual", job);
    started[0]?.reject(new Error("boom"));
    expect(await failing).toEqual({ status: "failed", error: "boom" });

    const next = coordinator.run("manual", job);
    started[1]?.resolve(result(1));
    expect((await next).status).toBe("done");
  });
});
