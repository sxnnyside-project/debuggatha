import { createReviewRequest, type ReviewRequest } from "@debuggatha/review-engine";
import { describe, expect, it } from "vitest";
import { fixturePack, fixtureRepositoryContext, fixtureRule } from "./internal/fixtures.js";
import { createInMemoryRegistry } from "./registry.js";
import { resolvePolicy } from "./resolve.js";

function requestFor(packIds: string[], requestedPolicyId?: string): ReviewRequest {
  return createReviewRequest({
    scope: { kind: "workspace" },
    depth: "full",
    requestedPackIds: packIds,
    ...(requestedPolicyId ? { requestedPolicyId } : {}),
  });
}

describe("resolvePolicy — composition", () => {
  it("unions every applicable pack rule with the repository's own criteria rules", () => {
    const pack = fixturePack({
      id: "react-ts",
      rules: [fixtureRule({ id: "no-any", packId: "react-ts" })],
    });
    const registry = createInMemoryRegistry([pack], []);
    const context = fixtureRepositoryContext({
      criteria: {
        rules: [
          {
            id: "no-console-log",
            description: "No console.log in production code.",
            source: { file: "CONTRIBUTING.md", detail: "heading" },
          },
        ],
        sources: ["CONTRIBUTING.md"],
      },
    });

    const policy = resolvePolicy(context, requestFor(["react-ts"]), registry);

    expect(policy.rules.map((r) => r.id).sort()).toEqual(["no-any", "no-console-log"]);
    expect(policy.conflicts).toEqual([]);
    expect(policy.packRefs).toEqual([{ id: "react-ts", version: "1.0.0" }]);
  });

  it("filters out pack rules whose appliesTo scope doesn't match the repository's stack", () => {
    const pack = fixturePack({
      id: "kotlin",
      rules: [
        fixtureRule({
          id: "kotlin-only",
          packId: "kotlin",
          appliesTo: { kind: "requires-language", language: "Kotlin" },
        }),
        fixtureRule({ id: "always-applies", packId: "kotlin", appliesTo: { kind: "always" } }),
      ],
    });
    const registry = createInMemoryRegistry([pack], []);
    // fixtureRepositoryContext's stack only declares TypeScript/React.
    const context = fixtureRepositoryContext();

    const policy = resolvePolicy(context, requestFor(["kotlin"]), registry);

    expect(policy.rules.map((r) => r.id)).toEqual(["always-applies"]);
  });

  it("matches a requires-framework rule against a registered framework signal", () => {
    const pack = fixturePack({
      id: "react-ts",
      rules: [
        fixtureRule({
          id: "hooks-rule",
          packId: "react-ts",
          appliesTo: { kind: "requires-framework", framework: "React" },
        }),
      ],
    });
    const registry = createInMemoryRegistry([pack], []);
    const context = fixtureRepositoryContext();

    const policy = resolvePolicy(context, requestFor(["react-ts"]), registry);
    expect(policy.rules.map((r) => r.id)).toEqual(["hooks-rule"]);
  });

  it("includes rules from a pack's transitive pack dependency", () => {
    const rust = fixturePack({
      id: "rust",
      rules: [fixtureRule({ id: "no-unwrap", packId: "rust" })],
    });
    const tauri = fixturePack({
      id: "tauri",
      rules: [fixtureRule({ id: "tauri-ipc", packId: "tauri" })],
      dependsOn: [{ packId: "rust", versionRange: "^1.0.0" }],
    });
    const registry = createInMemoryRegistry([rust, tauri], []);
    const context = fixtureRepositoryContext();

    const policy = resolvePolicy(context, requestFor(["tauri"]), registry);

    expect(policy.rules.map((r) => r.id).sort()).toEqual(["no-unwrap", "tauri-ipc"]);
    expect(policy.packRefs.map((p) => p.id).sort()).toEqual(["rust", "tauri"]);
  });
});

describe("resolvePolicy — conflicts (ADR-0003)", () => {
  it("resolves an explicit contradiction in favor of the repository's own criteria rule", () => {
    const pack = fixturePack({
      id: "react-ts",
      rules: [
        fixtureRule({
          id: "no-console-log",
          packId: "react-ts",
          contradicts: ["repo-allows-console"],
        }),
      ],
    });
    const registry = createInMemoryRegistry([pack], []);
    const context = fixtureRepositoryContext({
      criteria: {
        rules: [
          {
            id: "repo-allows-console",
            description: "console.log is allowed in this repo.",
            source: { file: "CONTRIBUTING.md", detail: "" },
          },
        ],
        sources: ["CONTRIBUTING.md"],
      },
    });

    const policy = resolvePolicy(context, requestFor(["react-ts"]), registry);

    expect(policy.rules.map((r) => r.id)).toEqual(["repo-allows-console"]);
    expect(policy.conflicts).toEqual([
      {
        winningRuleId: "repo-allows-console",
        losingRuleId: "no-console-log",
        reason:
          'rule "no-console-log" explicitly declares a contradiction with "repo-allows-console"',
      },
    ]);
  });

  it("never silently drops the losing rule — it stays traceable in conflicts", () => {
    const packA = fixturePack({
      id: "pack-a",
      rules: [fixtureRule({ id: "rule-a", packId: "pack-a", contradicts: ["rule-b"] })],
    });
    const packB = fixturePack({
      id: "pack-b",
      rules: [fixtureRule({ id: "rule-b", packId: "pack-b" })],
    });
    const registry = createInMemoryRegistry([packA, packB], []);
    const context = fixtureRepositoryContext();

    const policy = resolvePolicy(context, requestFor(["pack-a", "pack-b"]), registry);

    expect(policy.rules.map((r) => r.id)).toEqual(["rule-a"]);
    expect(policy.conflicts).toHaveLength(1);
    expect(policy.conflicts[0]).toMatchObject({ winningRuleId: "rule-a", losingRuleId: "rule-b" });
  });

  it("resolves a pack-vs-pack contradiction in favor of the pack requested first", () => {
    const packA = fixturePack({
      id: "pack-a",
      rules: [fixtureRule({ id: "rule-a", packId: "pack-a", contradicts: ["rule-b"] })],
    });
    const packB = fixturePack({
      id: "pack-b",
      rules: [fixtureRule({ id: "rule-b", packId: "pack-b" })],
    });
    const registry = createInMemoryRegistry([packA, packB], []);
    const context = fixtureRepositoryContext();

    const requestBFirst = requestFor(["pack-b", "pack-a"]);
    const policy = resolvePolicy(context, requestBFirst, registry);

    // pack-b was requested before pack-a, so rule-b wins even though rule-a declared the contradiction.
    expect(policy.rules.map((r) => r.id)).toEqual(["rule-b"]);
    expect(policy.conflicts[0]).toMatchObject({ winningRuleId: "rule-b", losingRuleId: "rule-a" });
  });

  it("auto-detects a pack rule id colliding with a repository criteria rule id, and the criteria rule wins", () => {
    const pack = fixturePack({
      id: "react-ts",
      rules: [fixtureRule({ id: "shared-id", packId: "react-ts" })],
    });
    const registry = createInMemoryRegistry([pack], []);
    const context = fixtureRepositoryContext({
      criteria: {
        rules: [
          {
            id: "shared-id",
            description: "The repo's own rule of the same name.",
            source: { file: "CONTRIBUTING.md", detail: "" },
          },
        ],
        sources: ["CONTRIBUTING.md"],
      },
    });

    const policy = resolvePolicy(context, requestFor(["react-ts"]), registry);

    expect(policy.rules).toHaveLength(1);
    expect(policy.rules[0]).toMatchObject({ id: "shared-id", origin: { kind: "criteria" } });
    expect(policy.conflicts[0]).toMatchObject({
      winningRuleId: "shared-id",
      losingRuleId: "shared-id",
    });
  });
});

describe("resolvePolicy — fail-closed dependency handling (Architecture doc §9)", () => {
  it("throws, naming the missing dependency, instead of silently assembling a partial policy", () => {
    const tauri = fixturePack({
      id: "tauri",
      dependsOn: [{ packId: "rust", versionRange: "^1.0.0" }],
    });
    const registry = createInMemoryRegistry([tauri], []);
    const context = fixtureRepositoryContext();

    expect(() => resolvePolicy(context, requestFor(["tauri"]), registry)).toThrow(/rust/);
  });

  it("throws when the top-level requested pack itself isn't registered", () => {
    const registry = createInMemoryRegistry([], []);
    const context = fixtureRepositoryContext();

    expect(() => resolvePolicy(context, requestFor(["ghost-pack"]), registry)).toThrow(
      /ghost-pack/,
    );
  });
});

describe("resolvePolicy — ReviewPolicy.id determinism (ADR-0005)", () => {
  it("produces the same id for two resolutions of the same inputs", () => {
    const pack = fixturePack({ id: "react-ts" });
    const registry = createInMemoryRegistry([pack], []);
    const context = fixtureRepositoryContext();
    const request = requestFor(["react-ts"]);

    const first = resolvePolicy(context, request, registry);
    const second = resolvePolicy(context, request, registry);

    expect(first.id).toBe(second.id);
  });

  it("produces a different id when the requested packs differ", () => {
    const packA = fixturePack({ id: "pack-a" });
    const packB = fixturePack({ id: "pack-b" });
    const registry = createInMemoryRegistry([packA, packB], []);
    const context = fixtureRepositoryContext();

    const policyA = resolvePolicy(context, requestFor(["pack-a"]), registry);
    const policyB = resolvePolicy(context, requestFor(["pack-b"]), registry);

    expect(policyA.id).not.toBe(policyB.id);
  });

  it("produces a different id when the repository's criteria differ", () => {
    const pack = fixturePack({ id: "react-ts" });
    const registry = createInMemoryRegistry([pack], []);
    const request = requestFor(["react-ts"]);

    const contextA = fixtureRepositoryContext({ criteria: { rules: [], sources: [] } });
    const contextB = fixtureRepositoryContext({
      criteria: {
        rules: [{ id: "x", description: "x", source: { file: "f", detail: "" } }],
        sources: ["f"],
      },
    });

    const policyA = resolvePolicy(contextA, request, registry);
    const policyB = resolvePolicy(contextB, request, registry);

    expect(policyA.id).not.toBe(policyB.id);
  });

  it("throws when a requestedPolicyId can't be reproduced from the current inputs", () => {
    const pack = fixturePack({ id: "react-ts" });
    const registry = createInMemoryRegistry([pack], []);
    const context = fixtureRepositoryContext();

    expect(() =>
      resolvePolicy(context, requestFor(["react-ts"], "not-the-real-id"), registry),
    ).toThrow(/does not match/);
  });

  it("succeeds when a requestedPolicyId matches the recomputed id", () => {
    const pack = fixturePack({ id: "react-ts" });
    const registry = createInMemoryRegistry([pack], []);
    const context = fixtureRepositoryContext();

    const firstRun = resolvePolicy(context, requestFor(["react-ts"]), registry);
    const rerun = resolvePolicy(context, requestFor(["react-ts"], firstRun.id), registry);

    expect(rerun.id).toBe(firstRun.id);
  });

  it("returns a frozen ReviewPolicy", () => {
    const pack = fixturePack({ id: "react-ts" });
    const registry = createInMemoryRegistry([pack], []);
    const context = fixtureRepositoryContext();

    const policy = resolvePolicy(context, requestFor(["react-ts"]), registry);
    expect(Object.isFrozen(policy)).toBe(true);
    expect(Object.isFrozen(policy.rules)).toBe(true);
  });
});
