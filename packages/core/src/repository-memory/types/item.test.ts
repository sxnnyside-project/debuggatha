import { describe, expect, it } from "bun:test";
import { createMemoryItem, transitionMemory } from "./item.js";

function fixtureInput() {
  return {
    category: "convention" as const,
    confidence: "documented" as const,
    rationale: "The team writes this down in CONTRIBUTING.md.",
    evidence: [{ file: "CONTRIBUTING.md", detail: "heading" }],
    rule: "Prefer composition over inheritance.",
    origin: { kind: "detection" as const, sessionId: "session-1" },
  };
}

describe("createMemoryItem", () => {
  it("always starts at detected, regardless of confidence", () => {
    const item = createMemoryItem(fixtureInput());
    expect(item.status).toBe("detected");
    expect(item.history).toHaveLength(1);
    expect(item.history[0]).toMatchObject({ previousState: undefined, newState: "detected" });
  });

  it("requires a rationale and evidence on every item (never opaque)", () => {
    const item = createMemoryItem(fixtureInput());
    expect(item.rationale.length).toBeGreaterThan(0);
    expect(item.evidence.length).toBeGreaterThan(0);
  });
});

describe("transitionMemory", () => {
  it("appends a history event and updates status", () => {
    const item = createMemoryItem(fixtureInput());
    const suggested = transitionMemory(item, "suggested", { kind: "detection", sessionId: "s" });
    expect(suggested.status).toBe("suggested");
    expect(suggested.history).toHaveLength(2);
    expect(suggested.history[1]).toMatchObject({
      previousState: "detected",
      newState: "suggested",
    });
  });

  it("throws on an illegal transition instead of silently allowing it", () => {
    const item = createMemoryItem(fixtureInput());
    expect(() => transitionMemory(item, "active", { kind: "user", actor: "alice" })).toThrow(
      /Illegal/,
    );
  });

  it("never mutates the original item (pure)", () => {
    const item = createMemoryItem(fixtureInput());
    const before = item.history.length;
    transitionMemory(item, "suggested", { kind: "detection", sessionId: "s" });
    expect(item.history.length).toBe(before);
    expect(item.status).toBe("detected");
  });
});
