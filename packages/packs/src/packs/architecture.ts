import type { ReviewPack } from "@debuggatha/core";

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
      limitations: [
        "A domain module importing a pure-language standard-library type (e.g. `crypto.randomUUID` in Node, or a language's own `Date`) is not an infrastructure dependency in the sense this rule targets — a naive import-path scan that flags any import outside the domain folder, rather than specifically framework/driver packages, will false-positive on these.",
      ],
    },
    {
      id: "know-arch-srp",
      title: "Single Responsibility Principle",
      body: "If a class handles HTTP parsing, SQL querying, and JSON serialization, changing the HTTP framework requires touching the SQL code. Separating concerns reduces the blast radius of modifications.",
      externalRefs: ["https://en.wikipedia.org/wiki/Single-responsibility_principle"],
      limitations: [
        '"One reason to change" is a judgment call about business-driven change axes, not file length or method count — a small, cohesive class with many methods that all change together for the same reason (e.g. a value object with several derived getters) is not an SRP violation, so a line-count or method-count heuristic alone produces false positives.',
      ],
    },
    {
      id: "know-arch-magic",
      title: "Magic Variables",
      body: "A literal like `86400` has no semantic meaning. A constant named `SECONDS_IN_DAY` clearly communicates intent and creates a single source of truth for the value.",
      externalRefs: ["https://en.wikipedia.org/wiki/Magic_number_(programming)"],
      limitations: [
        "Universally self-evident literals like `0`, `1`, `-1` (array bounds, increment/decrement, sentinel comparisons) or a `for` loop's index start are not magic numbers in the sense this rule targets — flagging every bare numeric literal regardless of whether its meaning is already obvious from context produces excessive false positives.",
      ],
    },
    {
      id: "know-arch-fail-fast",
      title: "Fail Fast Principle",
      body: "Allowing invalid data to propagate deep into a system before throwing an error makes debugging extremely difficult, as the stack trace originates far from the actual bug. Validating at the boundary stops corruption immediately.",
      externalRefs: ["https://en.wikipedia.org/wiki/Fail-fast"],
      limitations: [
        "A function that receives already-validated input from a caller in the same trust boundary (e.g. a private helper only ever invoked internally with a type the compiler already guarantees) does not need to re-validate — requiring boundary checks on every function regardless of whether it's actually a boundary produces redundant, noisy findings rather than catching a real gap.",
      ],
    },
  ],
};
