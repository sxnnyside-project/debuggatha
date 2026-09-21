import { afterEach, describe, expect, it } from "bun:test";
import { buildRepositoryContext } from "@debuggatha/repository-intelligence";
import { withTempRepo } from "@debuggatha/testing";
import { createReviewRequest } from "./request.js";
import { createReviewSession, transitionSession } from "./session.js";

function fixtureSession() {
  const repo = withTempRepo({ "README.md": "# Demo\n" });
  const repositoryContext = buildRepositoryContext(repo.root);
  const request = createReviewRequest({ scope: { kind: "workspace" }, depth: "full" });
  const session = createReviewSession({ request, repositoryContext });
  return { session, cleanup: repo.cleanup };
}

describe("createReviewSession", () => {
  let cleanup: (() => void) | undefined;

  afterEach(() => {
    cleanup?.();
    cleanup = undefined;
  });

  it("starts in the requested state, holding the RepositoryContext by reference", () => {
    const fixture = fixtureSession();
    cleanup = fixture.cleanup;

    expect(fixture.session.status).toBe("requested");
    expect(fixture.session.repositoryContext.documentation.sources).toHaveLength(1);
    expect(fixture.session.selectedPacks).toEqual([]);
  });
});

describe("transitionSession", () => {
  let cleanup: (() => void) | undefined;

  afterEach(() => {
    cleanup?.();
    cleanup = undefined;
  });

  it("walks the happy path and records execution metadata along the way", () => {
    const fixture = fixtureSession();
    cleanup = fixture.cleanup;

    const prepared = transitionSession(fixture.session, "prepared");
    const running = transitionSession(prepared, "running", {
      source: { kind: "deterministic-analyzer", name: "debuggatha-core" },
      startedAt: "2026-07-10T00:00:00.000Z",
    });
    const completed = transitionSession(running, "completed", {
      completedAt: "2026-07-10T00:00:01.000Z",
      durationMs: 1000,
    });

    expect(completed.status).toBe("completed");
    expect(completed.execution.source?.kind).toBe("deterministic-analyzer");
    expect(completed.execution.durationMs).toBe(1000);
  });

  it("never mutates the original session — each transition returns a new object", () => {
    const fixture = fixtureSession();
    cleanup = fixture.cleanup;

    const prepared = transitionSession(fixture.session, "prepared");

    expect(fixture.session.status).toBe("requested");
    expect(prepared).not.toBe(fixture.session);
  });

  it("throws on an illegal transition instead of silently invalid state", () => {
    const fixture = fixtureSession();
    cleanup = fixture.cleanup;

    expect(() => transitionSession(fixture.session, "completed")).toThrow(
      /Illegal review lifecycle transition/,
    );
  });

  it("records a human-review source just as validly as an ai-provider source", () => {
    const fixture = fixtureSession();
    cleanup = fixture.cleanup;

    const prepared = transitionSession(fixture.session, "prepared");
    const running = transitionSession(prepared, "running", {
      source: { kind: "human-review", name: "jane.doe" },
    });

    expect(running.execution.source).toEqual({ kind: "human-review", name: "jane.doe" });
  });
});
