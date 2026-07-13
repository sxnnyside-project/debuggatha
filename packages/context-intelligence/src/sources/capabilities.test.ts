import { buildRepositoryContext } from "@debuggatha/repository-intelligence";
import { withTempRepo } from "@debuggatha/testing";
import { afterEach, describe, expect, it } from "vitest";
import { buildCapabilityItems } from "./capabilities.js";

let cleanup: (() => void) | undefined;
afterEach(() => {
  cleanup?.();
  cleanup = undefined;
});

describe("buildCapabilityItems", () => {
  it("maps a high-confidence capability to detected, additively for every capability", () => {
    const repo = withTempRepo({
      "package.json": JSON.stringify({ name: "demo", devDependencies: { typescript: "^5.0.0" } }),
      "tsconfig.json": "{}",
    });
    cleanup = repo.cleanup;

    const context = buildRepositoryContext(repo.root);
    const items = buildCapabilityItems(context.capabilities);

    expect(items.length).toBe(context.capabilities.length);
    const ts = items.find((i) => i.capabilityId === "typescript");
    const js = items.find((i) => i.capabilityId === "javascript");
    expect(ts?.confidence).toBe("detected");
    expect(js?.confidence).toBe("detected");
    expect(ts?.category).toBe("capability");
  });

  it("maps a medium-confidence capability to inferred, never documented", () => {
    const repo = withTempRepo({
      "package.json": JSON.stringify({ name: "demo", devDependencies: { typescript: "^5.0.0" } }),
    });
    cleanup = repo.cleanup;

    const context = buildRepositoryContext(repo.root);
    const items = buildCapabilityItems(context.capabilities);
    const ts = items.find((i) => i.capabilityId === "typescript");
    expect(ts?.confidence).toBe("inferred");
  });
});
