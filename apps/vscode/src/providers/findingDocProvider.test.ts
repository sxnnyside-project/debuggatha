import type { Finding } from "@debuggatha/engine";
import { describe, expect, it } from "vitest";
import { FindingDocProvider } from "./findingDocProvider.js";

const finding = {
  id: "abc",
  title: "Avoid using the `any` type",
  explanation: "why",
  severity: "high",
  confidence: "medium",
  category: "maintainability",
  locations: [{ file: "/repo/src/a.ts", lines: { start: 7, end: 7 } }],
  evidence: [
    { kind: "code", file: "/repo/src/a.ts", lines: undefined, excerpt: "var x: any = 1;" },
    { kind: "criteria", ruleId: "no-explicit-any", source: "biome.json" },
    { kind: "review-pack-rule", packId: "debuggatha/typescript", ruleId: "no-explicit-any" },
  ],
  appliedPolicy: { id: "policy-1" },
  originatingPack: undefined,
  recommendations: [{ action: "fix", summary: "Use a specific type.", rationale: "Safer." }],
} as unknown as Finding;

describe("FindingDocProvider", () => {
  it("renders a registered finding as markdown a reviewer can act on", () => {
    const provider = new FindingDocProvider();
    const uri = provider.registerFinding(finding);
    expect(uri.scheme).toBe(FindingDocProvider.scheme);

    const md = provider.provideTextDocumentContent(uri);
    expect(md).toContain("# [HIGH] Avoid using the `any` type");
    expect(md).toContain("**Policy Reference**: `policy-1`");
    expect(md).toContain("**Location**: `/repo/src/a.ts:7`");
    expect(md).toContain("- var x: any = 1;");
    expect(md).toContain("- biome.json");
    expect(md).toContain("- review-pack-rule");
    expect(md).toContain("## Recommendation (fix)");
    expect(md).toContain("**Rationale**: Safer.");
  });

  it("says so when the finding was never registered", () => {
    const provider = new FindingDocProvider();
    expect(provider.provideTextDocumentContent({ path: "missing.md" } as never)).toBe(
      "# Finding Not Found",
    );
  });

  it("omits sections a finding does not have", () => {
    const provider = new FindingDocProvider();
    const bare = { ...finding, appliedPolicy: undefined, locations: [], recommendations: [] };
    const md = provider.provideTextDocumentContent(provider.registerFinding(bare as Finding));
    expect(md).not.toContain("Policy Reference");
    expect(md).not.toContain("Location");
    expect(md).not.toContain("Recommendation");
  });
});
