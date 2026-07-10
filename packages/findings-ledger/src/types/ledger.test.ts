import { describe, expect, it } from "vitest";
import { CURRENT_SCHEMA_VERSION, createEmptyLedger } from "./ledger.js";

describe("createEmptyLedger", () => {
  it("starts with no entries and the current schema version", () => {
    const ledger = createEmptyLedger("/repo");

    expect(ledger.entries).toEqual([]);
    expect(ledger.schemaVersion).toBe(CURRENT_SCHEMA_VERSION);
    expect(ledger.repositoryRoot).toBe("/repo");
    expect(ledger.createdAt).toBe(ledger.updatedAt);
  });
});
