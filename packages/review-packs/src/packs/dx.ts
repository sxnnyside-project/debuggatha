import type { ReviewPack } from "@debuggatha/knowledge-system";

export const dxPack: ReviewPack = {
  id: "debuggatha/dx",
  version: "1.0.0",
  kind: "concern",
  displayName: "Developer Experience",
  dependsOn: [],
  rules: [
    {
      id: "enforce-readme",
      packId: "debuggatha/dx",
      statement:
        "Every repository must have a `README.md` at the root containing local setup instructions, architecture overview, and test commands.",
      category: "style",
      appliesTo: { kind: "always" },
      defaultSeverity: "high",
      knowledgeRefs: ["know-dx-readme"],
      contradicts: undefined,
    },
    {
      id: "consistent-formatting",
      packId: "debuggatha/dx",
      statement:
        "A formatter (like Prettier, Biome, or rustfmt) must be configured and enforced via CI or pre-commit hooks to eliminate style debates.",
      category: "style",
      appliesTo: { kind: "always" },
      defaultSeverity: "medium",
      knowledgeRefs: ["know-dx-formatting"],
      contradicts: undefined,
    },
    {
      id: "meaningful-errors",
      packId: "debuggatha/dx",
      statement:
        "Error messages thrown in the codebase must contain actionable information. Avoid generic errors like `Something went wrong`.",
      category: "architecture",
      appliesTo: { kind: "always" },
      defaultSeverity: "low",
      knowledgeRefs: ["know-dx-errors"],
      contradicts: undefined,
    },
  ],
  knowledge: [
    {
      id: "know-dx-readme",
      title: "The README Contract",
      body: "The README is the entry point for every new engineer. Failing to document how to boot the local environment costs hours of lost productivity and builds tribal knowledge.",
      externalRefs: [],
    },
    {
      id: "know-dx-formatting",
      title: "Automated Formatting",
      body: "Debating tabs vs spaces or trailing commas in code reviews is a waste of engineering time. A strictly enforced auto-formatter standardizes the codebase and allows reviewers to focus purely on logic.",
      externalRefs: [],
    },
    {
      id: "know-dx-errors",
      title: "Actionable Error Messages",
      body: "When an engineer sees an error, they should immediately know what failed, why it failed, and what to do next. Generic error strings force the engineer to use a debugger to trace the failure origin.",
      externalRefs: [],
    },
  ],
};
