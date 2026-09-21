import { describe, expect, it } from "bun:test";
import { createFinding } from "../review-engine/index.js";
import { filterSuppressedFindings } from "./filter.js";
import type { SuppressionItem } from "./types/item.js";
import { createEmptyMemoryStore } from "./types/store.js";

function fixtureFinding(file: string, ruleId: string) {
  return createFinding({
    title: "t",
    explanation: "e",
    severity: "medium",
    confidence: "medium",
    category: "architecture",
    locations: [{ file, lines: undefined }],
    evidence: [{ kind: "review-pack-rule", packId: "p", ruleId }],
  });
}

function suppressionItem(status: SuppressionItem["status"]): SuppressionItem {
  return {
    id: "sup-1",
    category: "suppression",
    status,
    confidence: "user-confirmed",
    history: [],
    createdAt: "t",
    updatedAt: "t",
    rationale: "Known false positive in generated code.",
    evidence: [{ file: "src/a.ts", detail: "confirmed by alice" }],
    fingerprintFile: "src/a.ts",
    fingerprintRuleId: "no-var",
    fingerprintCategory: "architecture",
  };
}

describe("filterSuppressedFindings", () => {
  it("suppresses a finding matched by an active suppression, reporting it explicitly", () => {
    const store = { ...createEmptyMemoryStore("/repo"), items: [suppressionItem("active")] };
    const finding = fixtureFinding("src/a.ts", "no-var");

    const { findings, suppressed } = filterSuppressedFindings([finding], store);
    expect(findings).toEqual([]);
    expect(suppressed).toHaveLength(1);
    expect(suppressed[0]).toMatchObject({ memoryItemId: "sup-1", reason: "suppression" });
  });

  it("never suppresses using a non-active memory item (detected/suggested/confirmed/deprecated/archived)", () => {
    for (const status of [
      "detected",
      "suggested",
      "confirmed",
      "deprecated",
      "archived",
    ] as const) {
      const store = { ...createEmptyMemoryStore("/repo"), items: [suppressionItem(status)] };
      const finding = fixtureFinding("src/a.ts", "no-var");
      const { findings } = filterSuppressedFindings([finding], store);
      expect(findings).toEqual([finding]);
    }
  });

  it("keeps findings that don't match any active memory item", () => {
    const store = { ...createEmptyMemoryStore("/repo"), items: [suppressionItem("active")] };
    const unrelated = fixtureFinding("src/b.ts", "no-eval");
    const { findings, suppressed } = filterSuppressedFindings([unrelated], store);
    expect(findings).toEqual([unrelated]);
    expect(suppressed).toEqual([]);
  });

  it("returns everything unfiltered for an empty memory store", () => {
    const store = createEmptyMemoryStore("/repo");
    const finding = fixtureFinding("src/a.ts", "no-var");
    const { findings, suppressed } = filterSuppressedFindings([finding], store);
    expect(findings).toEqual([finding]);
    expect(suppressed).toEqual([]);
  });
});
