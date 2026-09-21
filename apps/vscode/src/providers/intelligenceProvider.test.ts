import {
  assemblePolicy,
  buildRepositoryContext,
  createReviewRequest,
  resolveRequestedPackIds,
} from "@debuggatha/engine";
import { afterAll, describe, expect, it } from "vitest";
import { makeRepo, removeRepos } from "../test/fixtures.js";
import { IntelligenceProvider } from "./intelligenceProvider.js";

afterAll(removeRepos);

function loaded() {
  const { root } = makeRepo();
  const context = buildRepositoryContext(root);
  const policy = assemblePolicy(
    context,
    createReviewRequest({
      scope: { kind: "workspace" },
      depth: "full",
      requestedPackIds: resolveRequestedPackIds([]),
    }),
  );
  const provider = new IntelligenceProvider();
  provider.refresh(context, policy);
  return { provider, context, policy };
}

describe("IntelligenceProvider", () => {
  it("shows nothing until a review has built a context", async () => {
    expect(await new IntelligenceProvider().getChildren()).toEqual([]);
  });

  it("offers the four sections of what Debuggatha understood", async () => {
    const { provider } = loaded();
    const roots = await provider.getChildren();
    expect(roots.map((node) => node.label)).toEqual([
      "Stack",
      "Documentation",
      "Criteria",
      "Active Review Policy",
    ]);
  });

  it("lists the detected stack under Stack", async () => {
    const { provider, context } = loaded();
    const [stack] = await provider.getChildren();
    const labels = (await provider.getChildren(stack)).map((node) => node.label);
    expect(labels.length).toBe(
      context.stack.languages.length +
        context.stack.frameworks.length +
        context.stack.buildSystems.length,
    );
    expect(labels.length).toBeGreaterThan(0);
  });

  it("lists every pack the policy resolved with its version", async () => {
    const { provider, policy } = loaded();
    const roots = await provider.getChildren();
    const policyNode = roots.find((node) => node.nodeId === "policy");
    const packs = await provider.getChildren(policyNode);
    expect(packs.map((node) => node.label)).toEqual(policy.packRefs.map((pack) => pack.id));
    expect(packs.every((node) => node.description)).toBe(true);
  });

  it("returns nothing beneath a leaf and notifies listeners on refresh", async () => {
    const { provider, context, policy } = loaded();
    let fired = 0;
    provider.onDidChangeTreeData(() => {
      fired += 1;
    });
    provider.refresh(context, policy);
    expect(fired).toBe(1);
    const [stack] = await provider.getChildren();
    const [leaf] = await provider.getChildren(stack);
    expect(await provider.getChildren(leaf)).toEqual([]);
  });
});
