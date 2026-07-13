import { describe, expect, it } from "vitest";
import {
  activateMemory,
  addMemoryItem,
  archiveMemory,
  confirmMemory,
  deprecateMemory,
  reactivateMemory,
  rejectMemory,
  removeMemoryItem,
  suggestMemory,
} from "./api.js";
import { createMemoryItem } from "./types/item.js";
import { createEmptyMemoryStore } from "./types/store.js";

function storeWithOneConvention() {
  const store = createEmptyMemoryStore("/repo");
  const item = createMemoryItem({
    category: "convention",
    confidence: "documented",
    rationale: "Written in CONTRIBUTING.md.",
    evidence: [{ file: "CONTRIBUTING.md", detail: "heading" }],
    rule: "No default exports.",
    origin: { kind: "detection", sessionId: "s" },
  });
  return { store: addMemoryItem(store, item), itemId: item.id };
}

describe("api lifecycle actions", () => {
  it("walks detected -> suggested -> confirmed -> active", () => {
    const { store, itemId } = storeWithOneConvention();
    const suggested = suggestMemory(store, itemId, "s1");
    expect(suggested.items[0]?.status).toBe("suggested");

    const confirmed = confirmMemory(suggested, itemId, "alice");
    expect(confirmed.items[0]?.status).toBe("confirmed");
    expect(confirmed.items[0]?.history.at(-1)?.origin).toEqual({ kind: "user", actor: "alice" });

    const active = activateMemory(confirmed, itemId, "alice");
    expect(active.items[0]?.status).toBe("active");
  });

  it("confirmMemory always records a user origin, never a detection origin", () => {
    const { store, itemId } = storeWithOneConvention();
    const suggested = suggestMemory(store, itemId, "s1");
    const confirmed = confirmMemory(suggested, itemId, "bob", "looks right");
    const lastEvent = confirmed.items[0]?.history.at(-1);
    expect(lastEvent?.origin.kind).toBe("user");
    expect(lastEvent?.comment).toBe("looks right");
  });

  it("rejectMemory sends a suggested item straight to archived", () => {
    const { store, itemId } = storeWithOneConvention();
    const suggested = suggestMemory(store, itemId, "s1");
    const rejected = rejectMemory(suggested, itemId, "alice", "not applicable");
    expect(rejected.items[0]?.status).toBe("archived");
  });

  it("deprecateMemory and reactivateMemory round-trip an active item", () => {
    const { store, itemId } = storeWithOneConvention();
    let s = suggestMemory(store, itemId, "s1");
    s = confirmMemory(s, itemId, "alice");
    s = activateMemory(s, itemId, "alice");
    const deprecated = deprecateMemory(s, itemId, "alice", "repo restructured");
    expect(deprecated.items[0]?.status).toBe("deprecated");
    const reactivated = reactivateMemory(deprecated, itemId, "alice");
    expect(reactivated.items[0]?.status).toBe("active");
  });

  it("archiveMemory is terminal — cannot transition further", () => {
    const { store, itemId } = storeWithOneConvention();
    const archived = archiveMemory(store, itemId, "alice");
    expect(archived.items[0]?.status).toBe("archived");
    expect(() => activateMemory(archived, itemId, "alice")).toThrow(/Illegal/);
  });

  it("removeMemoryItem deletes the item entirely, distinct from archiving", () => {
    const { store, itemId } = storeWithOneConvention();
    const removed = removeMemoryItem(store, itemId);
    expect(removed.items).toEqual([]);
  });

  it("throws a clear error for an operation on an unknown item id", () => {
    const { store } = storeWithOneConvention();
    expect(() => confirmMemory(store, "ghost", "alice")).toThrow(/ghost/);
  });

  it("every mutation is pure — never mutates the input store", () => {
    const { store, itemId } = storeWithOneConvention();
    const before = store.items[0]?.status;
    suggestMemory(store, itemId, "s1");
    expect(store.items[0]?.status).toBe(before);
  });
});
