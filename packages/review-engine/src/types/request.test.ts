import { describe, expect, it } from "vitest";
import { createReviewRequest } from "./request.js";

describe("createReviewRequest", () => {
  it("represents a Diff Review, a File Review, and a Workspace Review with the same domain object", () => {
    const diff = createReviewRequest({ scope: { kind: "diff", base: "main" }, depth: "full" });
    const files = createReviewRequest({
      scope: { kind: "files", paths: ["src/index.ts"] },
      depth: "quick",
    });
    const workspace = createReviewRequest({ scope: { kind: "workspace" }, depth: "architectural" });

    for (const request of [diff, files, workspace]) {
      expect(request).toHaveProperty("id");
      expect(request).toHaveProperty("scope");
      expect(request).toHaveProperty("depth");
      expect(request).toHaveProperty("requestedAt");
    }
    expect(diff.scope.kind).toBe("diff");
    expect(files.scope.kind).toBe("files");
    expect(workspace.scope.kind).toBe("workspace");
  });

  it("defaults diff.base to undefined, meaning 'since the last commit'", () => {
    const request = createReviewRequest({
      scope: { kind: "diff", base: undefined },
      depth: "quick",
    });
    expect(request.scope).toEqual({ kind: "diff", base: undefined });
  });

  it("refuses a file review with no files named", () => {
    expect(() =>
      createReviewRequest({ scope: { kind: "files", paths: [] }, depth: "quick" }),
    ).toThrow(/at least one file/);
  });

  it("generates a requestedAt timestamp when none is provided", () => {
    const request = createReviewRequest({ scope: { kind: "workspace" }, depth: "full" });
    expect(() => new Date(request.requestedAt).toISOString()).not.toThrow();
  });
});
