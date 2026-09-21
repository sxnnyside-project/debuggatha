import { describe, expect, it } from "bun:test";
import { canTransition, isTerminal, REVIEW_LIFECYCLE_STATUSES } from "./lifecycle.js";

describe("review lifecycle", () => {
  it("allows the happy path: requested -> prepared -> running -> completed", () => {
    expect(canTransition("requested", "prepared")).toBe(true);
    expect(canTransition("prepared", "running")).toBe(true);
    expect(canTransition("running", "completed")).toBe(true);
  });

  it("allows failing or cancelling from prepared or running", () => {
    expect(canTransition("prepared", "failed")).toBe(true);
    expect(canTransition("prepared", "cancelled")).toBe(true);
    expect(canTransition("running", "failed")).toBe(true);
    expect(canTransition("running", "cancelled")).toBe(true);
  });

  it("allows cancelling before preparation even starts", () => {
    expect(canTransition("requested", "cancelled")).toBe(true);
  });

  it("rejects skipping states", () => {
    expect(canTransition("requested", "running")).toBe(false);
    expect(canTransition("requested", "completed")).toBe(false);
  });

  it("rejects any transition out of a terminal state", () => {
    expect(canTransition("completed", "running")).toBe(false);
    expect(canTransition("failed", "running")).toBe(false);
    expect(canTransition("cancelled", "requested")).toBe(false);
  });

  it("treats completed, failed, and cancelled as terminal, and nothing else", () => {
    const terminal = REVIEW_LIFECYCLE_STATUSES.filter(isTerminal);
    expect(terminal.sort()).toEqual(["cancelled", "completed", "failed"]);
  });
});
