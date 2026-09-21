import { describe, expect, it, vi } from "bun:test";
import type { ReviewPolicy } from "@debuggatha/knowledge-system";
import type { RuntimeEngine } from "@debuggatha/runtime-engine";
import { runSemanticFindings } from "./semantic-findings.js";

const policy: ReviewPolicy = {
  id: "policy-1",
  rules: [
    {
      id: "r",
      statement: "Never use var.",
      category: "architecture",
      defaultSeverity: "high",
      origin: { kind: "pack", packId: "p", packVersion: "1.0.0" },
    },
  ],
  packRefs: [],
  conflicts: [],
};

function fakeRuntime(candidates: unknown[]): RuntimeEngine {
  return {
    listProviders: () => [],
    resolveRuntime: vi.fn(),
    streamSemanticReview: vi.fn(),
    runSemanticReview: vi.fn().mockResolvedValue({
      result: { candidates, parseWarnings: [] },
      metadata: {
        provider: "ollama",
        model: "llama3",
        contextWindow: 8192,
        durationMs: 10,
        estimatedPromptTokens: 100,
      },
    }),
  } as unknown as RuntimeEngine;
}

describe("runSemanticFindings", () => {
  it("converts a semantic candidate into a real, citable Finding", async () => {
    const runtime = fakeRuntime([
      {
        title: "Possible race condition",
        explanation: "Two async calls mutate shared state without synchronization.",
        confidence: "medium",
        severityHint: "high",
        evidence: [{ file: "src/a.ts", lines: { start: 5, end: 9 }, excerpt: "shared.value += 1" }],
        recommendation: "Guard the mutation with a lock or serialize the calls.",
      },
    ]);

    const findings = await runSemanticFindings(
      runtime,
      [{ file: "src/a.ts", content: "x", lineNumbers: undefined }],
      policy,
    );

    expect(findings).toHaveLength(1);
    expect(findings[0]?.severity).toBe("high");
    expect(findings[0]?.confidence).toBe("medium");
    expect(findings[0]?.appliedPolicy).toEqual({ id: "policy-1" });
    expect(findings[0]?.evidence[0]).toMatchObject({ kind: "code", file: "src/a.ts" });
    expect(findings[0]?.recommendations).toHaveLength(1);
    expect(findings[0]?.explanation).toContain("Semantic review finding");
  });

  it("defaults severity to medium when the model gave no severityHint", async () => {
    const runtime = fakeRuntime([
      {
        title: "t",
        explanation: "e",
        confidence: "low",
        severityHint: undefined,
        evidence: [{ file: "a.ts", lines: undefined, excerpt: "x" }],
        recommendation: undefined,
      },
    ]);
    const findings = await runSemanticFindings(
      runtime,
      [{ file: "a.ts", content: "x", lineNumbers: undefined }],
      policy,
    );
    expect(findings[0]?.severity).toBe("medium");
    expect(findings[0]?.recommendations).toEqual([]);
  });

  it("returns no findings when the runtime produces no candidates", async () => {
    const runtime = fakeRuntime([]);
    const findings = await runSemanticFindings(
      runtime,
      [{ file: "a.ts", content: "x", lineNumbers: undefined }],
      policy,
    );
    expect(findings).toEqual([]);
  });

  it("passes the policy's rule statements through to the runtime engine", async () => {
    const runSemanticReview = vi.fn().mockResolvedValue({
      result: { candidates: [], parseWarnings: [] },
      metadata: {
        provider: "ollama",
        model: "llama3",
        contextWindow: undefined,
        durationMs: 1,
        estimatedPromptTokens: 1,
      },
    });
    const runtime = {
      listProviders: () => [],
      resolveRuntime: vi.fn(),
      streamSemanticReview: vi.fn(),
      runSemanticReview,
    } as unknown as RuntimeEngine;

    await runSemanticFindings(
      runtime,
      [{ file: "a.ts", content: "x", lineNumbers: undefined }],
      policy,
    );

    expect(runSemanticReview).toHaveBeenCalledWith(
      expect.objectContaining({
        policyStatements: ["Never use var."],
        selection: { mode: "automatic" },
      }),
    );
  });
});
