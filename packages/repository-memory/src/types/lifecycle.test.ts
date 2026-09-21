import { describe, expect, it } from "bun:test";
import { canTransitionMemory, isActiveMemoryStatus } from "./lifecycle.js";

describe("canTransitionMemory", () => {
  it("allows the full detected -> ... -> archived path", () => {
    expect(canTransitionMemory("detected", "suggested")).toBe(true);
    expect(canTransitionMemory("suggested", "confirmed")).toBe(true);
    expect(canTransitionMemory("confirmed", "active")).toBe(true);
    expect(canTransitionMemory("active", "deprecated")).toBe(true);
    expect(canTransitionMemory("deprecated", "active")).toBe(true);
    expect(canTransitionMemory("active", "archived")).toBe(true);
  });

  it("allows rejecting straight from detected/suggested to archived", () => {
    expect(canTransitionMemory("detected", "archived")).toBe(true);
    expect(canTransitionMemory("suggested", "archived")).toBe(true);
  });

  it("rejects skipping states", () => {
    expect(canTransitionMemory("detected", "active")).toBe(false);
    expect(canTransitionMemory("detected", "confirmed")).toBe(false);
  });

  it("treats archived as terminal", () => {
    expect(canTransitionMemory("archived", "active")).toBe(false);
    expect(canTransitionMemory("archived", "detected")).toBe(false);
  });

  it("only active is an active memory status", () => {
    expect(isActiveMemoryStatus("active")).toBe(true);
    for (const s of ["detected", "suggested", "confirmed", "deprecated", "archived"] as const) {
      expect(isActiveMemoryStatus(s)).toBe(false);
    }
  });
});
