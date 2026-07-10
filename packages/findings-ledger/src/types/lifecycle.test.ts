import { describe, expect, it } from "vitest";
import {
  canTransitionFinding,
  FINDING_LIFECYCLE_STATUSES,
  isActiveFindingStatus,
} from "./lifecycle.js";

describe("finding lifecycle", () => {
  it("allows the happy path: open -> acknowledged -> resolved", () => {
    expect(canTransitionFinding("open", "acknowledged")).toBe(true);
    expect(canTransitionFinding("acknowledged", "resolved")).toBe(true);
  });

  it("allows a review to auto-resolve or a human to dismiss directly from open", () => {
    expect(canTransitionFinding("open", "resolved")).toBe(true);
    expect(canTransitionFinding("open", "dismissed")).toBe(true);
  });

  it("only allows resolved/dismissed findings to reopen, never open/acknowledged directly", () => {
    expect(canTransitionFinding("resolved", "reopened")).toBe(true);
    expect(canTransitionFinding("dismissed", "reopened")).toBe(true);
    expect(canTransitionFinding("open", "reopened")).toBe(false);
    expect(canTransitionFinding("acknowledged", "reopened")).toBe(false);
  });

  it("lets a reopened finding follow the same paths as open", () => {
    expect(canTransitionFinding("reopened", "acknowledged")).toBe(true);
    expect(canTransitionFinding("reopened", "resolved")).toBe(true);
    expect(canTransitionFinding("reopened", "dismissed")).toBe(true);
  });

  it("rejects skipping states or transitioning out of a status with no legal moves defined", () => {
    expect(canTransitionFinding("resolved", "open")).toBe(false);
    expect(canTransitionFinding("dismissed", "acknowledged")).toBe(false);
  });

  it("treats open, acknowledged, and reopened as the active statuses eligible for re-matching", () => {
    const active = FINDING_LIFECYCLE_STATUSES.filter(isActiveFindingStatus);
    expect(active.sort()).toEqual(["acknowledged", "open", "reopened"]);
  });
});
