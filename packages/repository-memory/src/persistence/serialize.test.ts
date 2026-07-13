import { describe, expect, it } from "vitest";
import { createEmptyMemoryStore } from "../types/store.js";
import { deserializeMemoryStore, serializeMemoryStore } from "./serialize.js";

describe("serializeMemoryStore / deserializeMemoryStore", () => {
  it("round-trips an empty store", () => {
    const store = createEmptyMemoryStore("/repo");
    const json = serializeMemoryStore(store);
    const restored = deserializeMemoryStore(json);
    expect(restored.repositoryRoot).toBe("/repo");
    expect(restored.items).toEqual([]);
  });

  it("produces deterministic (sorted-key) output regardless of key insertion order", () => {
    const a = serializeMemoryStore({
      schemaVersion: 1,
      repositoryRoot: "/r",
      items: [],
      createdAt: "t",
      updatedAt: "t",
    });
    const b = serializeMemoryStore({
      updatedAt: "t",
      createdAt: "t",
      items: [],
      repositoryRoot: "/r",
      schemaVersion: 1,
    });
    expect(a).toBe(b);
  });

  it("fails closed on an unsupported schema version instead of guessing", () => {
    const json = JSON.stringify({ schemaVersion: 999, repositoryRoot: "/r", items: [] });
    expect(() => deserializeMemoryStore(json)).toThrow(/schema version 999/);
  });

  it("fails closed when schemaVersion is missing entirely", () => {
    expect(() => deserializeMemoryStore(JSON.stringify({ items: [] }))).toThrow(/schemaVersion/);
  });

  it("returns a frozen store", () => {
    const restored = deserializeMemoryStore(serializeMemoryStore(createEmptyMemoryStore("/repo")));
    expect(Object.isFrozen(restored)).toBe(true);
  });
});
