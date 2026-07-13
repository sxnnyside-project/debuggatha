import { describe, expect, it } from "vitest";
import type { Provider } from "./provider.js";
import { createProviderRegistry } from "./registry.js";

function stubProvider(id: string): Provider {
  return {
    id,
    displayName: id,
    discoverModels: async () => [],
    healthCheck: async () => ({ available: true, detail: "" }),
    contextWindowFor: async () => undefined,
    execute: async function* () {},
  };
}

describe("createProviderRegistry", () => {
  it("registers providers at construction and via register()", () => {
    const registry = createProviderRegistry([stubProvider("a")]);
    registry.register(stubProvider("b"));
    expect(
      registry
        .list()
        .map((p) => p.id)
        .sort(),
    ).toEqual(["a", "b"]);
  });

  it("get() returns undefined for an unregistered id", () => {
    const registry = createProviderRegistry();
    expect(registry.get("ghost")).toBeUndefined();
  });

  it("registering a provider with an existing id replaces it, not adds a duplicate", () => {
    const registry = createProviderRegistry([stubProvider("a")]);
    const replacement = stubProvider("a");
    registry.register(replacement);
    expect(registry.list()).toHaveLength(1);
    expect(registry.get("a")).toBe(replacement);
  });
});
