import type { ReviewPack } from "@debuggatha/knowledge-system";

export const architecturePack: ReviewPack = {
  id: "debuggatha/architecture",
  version: "1.0.0",
  kind: "concern",
  displayName: "Architecture",
  dependsOn: [],
  rules: [
    {
      id: "dependency-inversion",
      packId: "debuggatha/architecture",
      statement:
        "Domain logic (entities, use cases) must not import or depend on infrastructure logic (database drivers, HTTP clients, UI frameworks).",
      category: "architecture",
      appliesTo: { kind: "always" },
      defaultSeverity: "critical",
      knowledgeRefs: ["know-arch-dip"],
      contradicts: undefined,
    },
    {
      id: "single-responsibility",
      packId: "debuggatha/architecture",
      statement:
        "Classes and modules should have exactly one reason to change. Separate data fetching from presentation.",
      category: "architecture",
      appliesTo: { kind: "always" },
      defaultSeverity: "medium",
      knowledgeRefs: ["know-arch-srp"],
      contradicts: undefined,
    },
    {
      id: "no-magic-numbers",
      packId: "debuggatha/architecture",
      statement:
        "Do not use magic numbers or raw strings in domain logic. Define them as explicit named constants or enums.",
      category: "style",
      appliesTo: { kind: "always" },
      defaultSeverity: "low",
      knowledgeRefs: ["know-arch-magic"],
      contradicts: undefined,
    },
    {
      id: "fail-fast",
      packId: "debuggatha/architecture",
      statement:
        "Validate invariants and arguments at the boundary of a function or class. Throw an error immediately rather than proceeding with invalid state.",
      category: "architecture",
      appliesTo: { kind: "always" },
      defaultSeverity: "high",
      knowledgeRefs: ["know-arch-fail-fast"],
      contradicts: undefined,
    },
  ],
  knowledge: [
    {
      id: "know-arch-dip",
      title: "Dependency Inversion Principle",
      body: "High-level modules should not depend on low-level modules. Both should depend on abstractions. Injecting a database dependency via an interface allows the domain logic to be tested in isolation and the database to be swapped without rewriting business rules.",
      externalRefs: ["https://en.wikipedia.org/wiki/Dependency_inversion_principle"],
    },
    {
      id: "know-arch-srp",
      title: "Single Responsibility Principle",
      body: "If a class handles HTTP parsing, SQL querying, and JSON serialization, changing the HTTP framework requires touching the SQL code. Separating concerns reduces the blast radius of modifications.",
      externalRefs: ["https://en.wikipedia.org/wiki/Single-responsibility_principle"],
    },
    {
      id: "know-arch-magic",
      title: "Magic Variables",
      body: "A literal like `86400` has no semantic meaning. A constant named `SECONDS_IN_DAY` clearly communicates intent and creates a single source of truth for the value.",
      externalRefs: ["https://en.wikipedia.org/wiki/Magic_number_(programming)"],
    },
    {
      id: "know-arch-fail-fast",
      title: "Fail Fast Principle",
      body: "Allowing invalid data to propagate deep into a system before throwing an error makes debugging extremely difficult, as the stack trace originates far from the actual bug. Validating at the boundary stops corruption immediately.",
      externalRefs: ["https://en.wikipedia.org/wiki/Fail-fast"],
    },
  ],
};
