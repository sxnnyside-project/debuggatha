import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterAll, describe, expect, it } from "vitest";
import { createInProcessRunner, createProcessRunner } from "./process-runner.js";
import { ReviewCancelled, type ReviewJob } from "./protocol.js";

const dir = mkdtempSync(join(tmpdir(), "debuggatha-runner-"));
afterAll(() => rmSync(dir, { recursive: true, force: true }));

const job: ReviewJob = {
  rootDir: dir,
  scope: { kind: "workspace" },
  packIds: [],
  analyzers: { mode: "off" },
};
const RESULT = {
  findings: 3,
  analyzers: [],
  semantic: null,
  changes: { introduced: 1, fixed: 0, reopened: 0 },
  durationMs: 5,
};

/** A stand-in for `dist/review-worker.js`, so the process handling is tested on its own. */
function worker(name: string, body: string): string {
  const path = join(dir, `${name}.js`);
  writeFileSync(path, body);
  return path;
}

const alive = (pid: number) => {
  try {
    process.kill(pid, 0);
    return true;
  } catch {
    return false;
  }
};
const waitFor = async (condition: () => boolean, ms = 5_000) => {
  const until = Date.now() + ms;
  while (!condition()) {
    if (Date.now() > until) throw new Error("timed out waiting");
    await new Promise((resolve) => setTimeout(resolve, 25));
  }
};

describe("createProcessRunner", () => {
  it("hands the job to another process and returns what it reports", async () => {
    const path = worker(
      "echo",
      `process.once("message", (m) => process.send({ type: "done", result: { ...${JSON.stringify(RESULT)}, findings: m.job.packIds.length + 3 } }, () => process.exit(0)));`,
    );
    const result = await createProcessRunner(path).run(
      { ...job, packIds: ["a", "b"] },
      new AbortController().signal,
    );
    expect(result.findings).toBe(5);
  });

  it("reports the reason when the review fails", async () => {
    const path = worker(
      "fail",
      `process.once("message", () => process.send({ type: "error", message: "not a repository" }, () => process.exit(1)));`,
    );
    await expect(createProcessRunner(path).run(job, new AbortController().signal)).rejects.toThrow(
      "not a repository",
    );
  });

  it("says so, with what it printed, when the process dies without answering", async () => {
    const path = worker(
      "crash",
      `process.once("message", () => { console.error("out of memory"); process.exit(3); });`,
    );
    const failure = createProcessRunner(path).run(job, new AbortController().signal);
    await expect(failure).rejects.toThrow("code 3");
    await expect(failure).rejects.toThrow("out of memory");
  });

  it("does not start when it is already cancelled", async () => {
    const controller = new AbortController();
    controller.abort();
    await expect(
      createProcessRunner(join(dir, "never.js")).run(job, controller.signal),
    ).rejects.toBeInstanceOf(ReviewCancelled);
  });

  it("cancelling stops the review and the analyzer it started, not just the review", async () => {
    const pids = join(dir, "pids");
    mkdirSync(pids, { recursive: true });
    const path = worker(
      "slow",
      `
      const { spawn } = require("node:child_process");
      const { writeFileSync } = require("node:fs");
      process.once("message", () => {
        // An analyzer: a child of the review that would run for a long time.
        const analyzer = spawn("sleep", ["60"], { stdio: "ignore" });
        writeFileSync(${JSON.stringify(join(pids, "analyzer"))}, String(analyzer.pid));
        writeFileSync(${JSON.stringify(join(pids, "review"))}, String(process.pid));
        setInterval(() => {}, 1000);
      });`,
    );
    const controller = new AbortController();
    const running = createProcessRunner(path).run(job, controller.signal);
    const outcome = running.then(
      () => "finished",
      (error) => error,
    );

    await waitFor(() => {
      try {
        return readFileSync(join(pids, "analyzer"), "utf8") !== "";
      } catch {
        return false;
      }
    });
    const review = Number(readFileSync(join(pids, "review"), "utf8"));
    const analyzer = Number(readFileSync(join(pids, "analyzer"), "utf8"));
    expect(alive(review) && alive(analyzer)).toBe(true);

    controller.abort();
    expect(await outcome).toBeInstanceOf(ReviewCancelled);
    await waitFor(() => !alive(review) && !alive(analyzer));
  });
});

describe("createInProcessRunner", () => {
  it("runs the engine and returns its summary", async () => {
    const root = mkdtempSync(join(tmpdir(), "debuggatha-inproc-"));
    try {
      writeFileSync(join(root, "package.json"), JSON.stringify({ name: "x" }));
      writeFileSync(join(root, "a.js"), "var x = 1;\n");
      const result = await createInProcessRunner().run(
        { ...job, rootDir: root },
        new AbortController().signal,
      );
      expect(result.findings).toBeGreaterThan(0);
      expect(result.analyzers).toEqual([]);
      expect(result.semantic).toBeNull();
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  });

  it("does not run when cancelled", async () => {
    const controller = new AbortController();
    controller.abort();
    await expect(createInProcessRunner().run(job, controller.signal)).rejects.toBeInstanceOf(
      ReviewCancelled,
    );
  });
});
