import { describe, expect, it } from "vitest";
import { parseSemanticResponse } from "./parse-response.js";

const validCandidate = {
  title: "Possible SQL injection",
  explanation: "User input is concatenated directly into a query string.",
  confidence: "high",
  severityHint: "critical",
  evidence: [{ file: "src/db.ts", lines: { start: 10, end: 12 }, excerpt: "query + userInput" }],
  recommendation: "Use a parameterized query.",
};

describe("parseSemanticResponse", () => {
  it("parses a well-formed JSON response", () => {
    const result = parseSemanticResponse(JSON.stringify({ candidates: [validCandidate] }));
    expect(result.candidates).toHaveLength(1);
    expect(result.candidates[0]).toMatchObject({ title: validCandidate.title, confidence: "high" });
    expect(result.parseWarnings).toEqual([]);
  });

  it("extracts JSON from a markdown code fence", () => {
    const fenced =
      "Here you go:\n```json\n" + JSON.stringify({ candidates: [validCandidate] }) + "\n```";
    const result = parseSemanticResponse(fenced);
    expect(result.candidates).toHaveLength(1);
  });

  it("returns no candidates and a warning for text with no JSON at all", () => {
    const result = parseSemanticResponse("I don't see any issues in this code.");
    expect(result.candidates).toEqual([]);
    expect(result.parseWarnings.length).toBeGreaterThan(0);
  });

  it("returns no candidates and a warning for malformed JSON", () => {
    const result = parseSemanticResponse("{ candidates: [ this is not valid json }");
    expect(result.candidates).toEqual([]);
    expect(result.parseWarnings[0]).toMatch(/failed to parse/);
  });

  it("drops a candidate with no evidence instead of fabricating a location", () => {
    const noEvidence = { ...validCandidate, evidence: [] };
    const result = parseSemanticResponse(JSON.stringify({ candidates: [noEvidence] }));
    expect(result.candidates).toEqual([]);
    expect(result.parseWarnings[0]).toMatch(/dropped/);
  });

  it("drops a candidate missing title/explanation", () => {
    const result = parseSemanticResponse(
      JSON.stringify({ candidates: [{ evidence: validCandidate.evidence }] }),
    );
    expect(result.candidates).toEqual([]);
  });

  it("defaults confidence to low and severityHint to undefined for invalid values", () => {
    const result = parseSemanticResponse(
      JSON.stringify({
        candidates: [{ ...validCandidate, confidence: "extreme", severityHint: "apocalyptic" }],
      }),
    );
    expect(result.candidates[0]?.confidence).toBe("low");
    expect(result.candidates[0]?.severityHint).toBeUndefined();
  });

  it("handles an empty candidates array", () => {
    const result = parseSemanticResponse(JSON.stringify({ candidates: [] }));
    expect(result.candidates).toEqual([]);
    expect(result.parseWarnings).toEqual([]);
  });
});
