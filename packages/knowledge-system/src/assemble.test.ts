import { createReviewRequest } from "@debuggatha/review-engine";
import { describe, expect, it } from "vitest";
import { assembleContext } from "./assemble.js";
import { fixturePack, fixtureRepositoryContext } from "./internal/fixtures.js";
import { createInMemoryRegistry } from "./registry.js";
import { resolvePolicy } from "./resolve.js";

describe("assembleContext", () => {
  it("assembles a SkillContext with policy: undefined for a Core Skill", () => {
    const context = fixtureRepositoryContext();
    const request = createReviewRequest({ scope: { kind: "workspace" }, depth: "full" });

    const skillContext = assembleContext(context, request, undefined);

    expect(skillContext.repository).toBe(context);
    expect(skillContext.request).toBe(request);
    expect(skillContext.policy).toBeUndefined();
  });

  it("assembles a SkillContext with a resolved ReviewPolicy for a Review/Analysis Skill", () => {
    const pack = fixturePack({ id: "react-ts" });
    const registry = createInMemoryRegistry([pack], []);
    const context = fixtureRepositoryContext();
    const request = createReviewRequest({
      scope: { kind: "diff", base: "main" },
      depth: "architectural",
      requestedPackIds: ["react-ts"],
    });

    const policy = resolvePolicy(context, request, registry);
    const skillContext = assembleContext(context, request, policy);

    expect(skillContext.policy).toBe(policy);
    expect(skillContext.policy?.rules.length).toBeGreaterThan(0);
  });

  it("returns a shallow-frozen container without mutating the caller's request", () => {
    const context = fixtureRepositoryContext();
    const request = createReviewRequest({ scope: { kind: "workspace" }, depth: "quick" });

    const skillContext = assembleContext(context, request, undefined);

    expect(Object.isFrozen(skillContext)).toBe(true);
    expect(Object.isFrozen(request)).toBe(false);
  });
});
