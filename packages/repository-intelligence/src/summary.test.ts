import { describe, expect, it } from "vitest";
import { summarizeCapabilities } from "./summary.js";
import type { Capability } from "./types.js";

function cap(id: string, kind: Capability["kind"]): Capability {
  return { id, kind, confidence: "high", evidence: [], origin: "manifest" };
}

describe("summarizeCapabilities", () => {
  it("reports no capabilities for an empty set", () => {
    expect(summarizeCapabilities([])).toBe("No capabilities detected.");
  });

  it("groups by kind and applies known display overrides deterministically", () => {
    const summary = summarizeCapabilities([
      cap("typescript", "language"),
      cap("javascript", "language"),
      cap("react", "framework"),
      cap("bun", "tooling"),
    ]);
    expect(summary).toBe("Languages: JavaScript, TypeScript — Frameworks: React — Tooling: Bun");
  });

  it("produces the same summary regardless of input order (deterministic)", () => {
    const a = [cap("react", "framework"), cap("typescript", "language")];
    const b = [cap("typescript", "language"), cap("react", "framework")];
    expect(summarizeCapabilities(a)).toBe(summarizeCapabilities(b));
  });
});
