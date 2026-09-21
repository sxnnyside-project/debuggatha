import { describe, expect, it } from "bun:test";
import type {
  CriteriaProfile,
  DependencyProfile,
  DocumentationProfile,
  StackProfile,
} from "../types.js";
import { deriveUnderstanding } from "./understanding.js";

const emptyStack: StackProfile = {
  languages: [],
  frameworks: [],
  buildSystems: [],
  packageManagers: [],
  runtimes: [],
  platformTargets: [],
  workspaceType: "single-package",
  workspaceEvidence: [],
  manifests: [],
};

const emptyDependencies: DependencyProfile = {
  lockfiles: [],
  declaredVersions: {},
  runtimeConstraints: {},
};
const emptyDocumentation: DocumentationProfile = {
  sources: [],
  summary: "No documentation sources found.",
};
const emptyCriteria: CriteriaProfile = { rules: [], sources: [] };

describe("deriveUnderstanding", () => {
  it("is high confidence with no open questions when everything was found", () => {
    const stack: StackProfile = {
      ...emptyStack,
      languages: [{ value: "Rust", evidence: [{ file: "Cargo.toml", detail: "present" }] }],
      runtimes: [{ value: "Bun", evidence: [{ file: "bun.lock", detail: "present" }] }],
    };
    const documentation: DocumentationProfile = {
      sources: [{ file: "README.md", kind: "readme", title: "Demo", wordCount: 10 }],
      summary: "1 documentation source found: 1 readme.",
    };
    const criteria: CriteriaProfile = {
      rules: [
        {
          id: "eslint:eqeqeq",
          description: 'ESLint rule "eqeqeq" configured',
          source: { file: ".eslintrc.json", detail: '"error"' },
        },
      ],
      sources: [".eslintrc.json"],
    };

    const result = deriveUnderstanding(stack, emptyDependencies, documentation, criteria);

    expect(result.confidence).toBe("high");
    expect(result.openQuestions).toHaveLength(0);
  });

  it("asks what language the project uses when nothing was detected, never guessing", () => {
    const result = deriveUnderstanding(
      emptyStack,
      emptyDependencies,
      emptyDocumentation,
      emptyCriteria,
    );

    expect(result.openQuestions.map((q) => q.question)).toContain(
      "What language(s) does this project use?",
    );
  });

  it("asks about the JS/TS runtime when package.json exists but no runtime marker was found", () => {
    const stack: StackProfile = {
      ...emptyStack,
      languages: [
        { value: "JavaScript/TypeScript", evidence: [{ file: "package.json", detail: "present" }] },
      ],
    };

    const result = deriveUnderstanding(stack, emptyDependencies, emptyDocumentation, emptyCriteria);

    expect(result.openQuestions.map((q) => q.question)).toContain(
      "Which JavaScript/TypeScript runtime does this project target (Node.js, Bun, Deno)?",
    );
  });

  it("degrades confidence to low once three or more questions accumulate", () => {
    const result = deriveUnderstanding(
      emptyStack,
      emptyDependencies,
      emptyDocumentation,
      emptyCriteria,
    );

    expect(result.openQuestions.length).toBeGreaterThanOrEqual(3);
    expect(result.confidence).toBe("low");
  });
});
