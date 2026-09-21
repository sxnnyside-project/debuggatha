import { afterEach, describe, expect, it } from "bun:test";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { addMemoryItem } from "../api.js";
import { createMemoryItem } from "../types/item.js";
import { loadMemoryStore, memoryFilePath, saveMemoryStore } from "./fs.js";

let dir: string | undefined;
afterEach(() => {
  if (dir) rmSync(dir, { recursive: true, force: true });
  dir = undefined;
});

describe("loadMemoryStore / saveMemoryStore", () => {
  it("returns an empty store for a repository with no memory file yet", () => {
    dir = mkdtempSync(join(tmpdir(), "debuggatha-memory-"));
    const store = loadMemoryStore(dir);
    expect(store.items).toEqual([]);
    expect(store.repositoryRoot).toBe(dir);
  });

  it("persists at .debuggatha/memory.json and round-trips", () => {
    dir = mkdtempSync(join(tmpdir(), "debuggatha-memory-"));
    const store = loadMemoryStore(dir);
    const item = createMemoryItem({
      category: "convention",
      confidence: "documented",
      rationale: "Written in CONTRIBUTING.md.",
      evidence: [{ file: "CONTRIBUTING.md", detail: "heading" }],
      rule: "No default exports.",
      origin: { kind: "detection", sessionId: "s" },
    });
    const updated = addMemoryItem(store, item);
    saveMemoryStore(updated);

    expect(memoryFilePath(dir)).toBe(join(dir, ".debuggatha", "memory.json"));
    const reloaded = loadMemoryStore(dir);
    expect(reloaded.items).toHaveLength(1);
    expect(reloaded.items[0]?.id).toBe(item.id);
  });
});
