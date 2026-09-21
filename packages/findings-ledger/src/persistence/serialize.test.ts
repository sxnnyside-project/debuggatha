import { describe, expect, it } from "bun:test";
import { CURRENT_SCHEMA_VERSION, createEmptyLedger } from "../types/ledger.js";
import { deserializeLedger, serializeLedger } from "./serialize.js";

describe("serializeLedger / deserializeLedger", () => {
  it("round-trips an empty ledger", () => {
    const ledger = createEmptyLedger("/repo");
    const restored = deserializeLedger(serializeLedger(ledger));
    expect(restored).toEqual(ledger);
  });

  it("is deterministic regardless of key construction order", () => {
    const a = {
      schemaVersion: 1,
      repositoryRoot: "/repo",
      entries: [],
      createdAt: "t",
      updatedAt: "t",
    };
    const b = {
      updatedAt: "t",
      entries: [],
      createdAt: "t",
      repositoryRoot: "/repo",
      schemaVersion: 1,
    };

    expect(serializeLedger(a as never)).toBe(serializeLedger(b as never));
  });

  it("produces human-readable, indented JSON", () => {
    const serialized = serializeLedger(createEmptyLedger("/repo"));
    expect(serialized).toContain("\n  ");
  });

  it("rejects a payload with no schemaVersion", () => {
    expect(() => deserializeLedger(JSON.stringify({ entries: [] }))).toThrow(/schemaVersion/);
  });

  it("fails closed on an unrecognized schema version instead of guessing", () => {
    const future = JSON.stringify({
      ...createEmptyLedger("/repo"),
      schemaVersion: CURRENT_SCHEMA_VERSION + 1,
    });
    expect(() => deserializeLedger(future)).toThrow(/not supported/);
  });

  it("returns a deeply frozen ledger", () => {
    const restored = deserializeLedger(serializeLedger(createEmptyLedger("/repo")));
    expect(Object.isFrozen(restored)).toBe(true);
  });
});
