import { describe, expect, it } from "vitest";
import type { CacheEntry } from "./index.js";
import { createInMemoryCache } from "./index.js";

describe("createInMemoryCache", () => {
  it("returns undefined for a root that was never cached", () => {
    const cache = createInMemoryCache();
    expect(cache.get("/nonexistent")).toBeUndefined();
  });

  it("returns exactly what was stored for a given root, and keeps separate roots isolated", () => {
    const cache = createInMemoryCache();
    const entryA = { context: { rootDir: "/a" } } as unknown as CacheEntry;
    const entryB = { context: { rootDir: "/b" } } as unknown as CacheEntry;

    cache.set("/a", entryA);
    cache.set("/b", entryB);

    expect(cache.get("/a")).toBe(entryA);
    expect(cache.get("/b")).toBe(entryB);
  });
});
