import type { ReviewPack } from "@debuggatha/knowledge-system";

export const typescriptPack: ReviewPack = {
  id: "debuggatha/typescript",
  version: "1.0.0",
  kind: "stack",
  displayName: "TypeScript",
  dependsOn: [],
  rules: [
    {
      id: "no-explicit-any",
      packId: "debuggatha/typescript",
      statement:
        "Avoid using the `any` type. Use `unknown` if the type is truly dynamic, and apply type narrowing.",
      category: "architecture",
      appliesTo: { kind: "requires-language", language: "typescript" },
      defaultSeverity: "high",
      knowledgeRefs: ["know-typescript-any"],
      contradicts: undefined,
    },
    {
      id: "strict-null-checks",
      packId: "debuggatha/typescript",
      statement:
        "Do not use the non-null assertion operator (`!`). Handle nullability explicitly with optional chaining (`?.`) or control flow narrowing.",
      category: "architecture",
      appliesTo: { kind: "requires-language", language: "typescript" },
      defaultSeverity: "high",
      knowledgeRefs: ["know-typescript-nullability"],
      contradicts: undefined,
    },
    {
      id: "no-floating-promises",
      packId: "debuggatha/typescript",
      statement:
        "Promises must be explicitly awaited, returned, or have `.catch()` called on them.",
      category: "security",
      appliesTo: { kind: "requires-language", language: "typescript" },
      defaultSeverity: "critical",
      knowledgeRefs: ["know-typescript-promises"],
      contradicts: undefined,
    },
    {
      id: "consistent-type-imports",
      packId: "debuggatha/typescript",
      statement:
        "Use `import type` for types to ensure they are fully erased during compilation and do not cause side effects in bundlers.",
      category: "performance",
      appliesTo: { kind: "requires-language", language: "typescript" },
      defaultSeverity: "low",
      knowledgeRefs: ["know-typescript-type-imports"],
      contradicts: undefined,
    },
    {
      id: "explicit-return-types",
      packId: "debuggatha/typescript",
      statement:
        "Exported module functions must have explicit return types rather than relying on inference.",
      category: "architecture",
      appliesTo: { kind: "requires-language", language: "typescript" },
      defaultSeverity: "medium",
      knowledgeRefs: ["know-typescript-return-types"],
      contradicts: undefined,
    },
    {
      id: "prefer-readonly",
      packId: "debuggatha/typescript",
      statement:
        "Mark properties, arrays, and tuples as `readonly` when they are not intended to be mutated.",
      category: "architecture",
      appliesTo: { kind: "requires-language", language: "typescript" },
      defaultSeverity: "medium",
      knowledgeRefs: ["know-typescript-immutability"],
      contradicts: undefined,
    },
    {
      id: "prefer-nullish-coalescing",
      packId: "debuggatha/typescript",
      statement:
        "Use the nullish coalescing operator (`??`) instead of logical OR (`||`) when providing default values for nullable variables.",
      category: "security",
      appliesTo: { kind: "requires-language", language: "typescript" },
      defaultSeverity: "medium",
      knowledgeRefs: ["know-typescript-nullish"],
      contradicts: undefined,
    },
    {
      id: "no-unsafe-enum-assignment",
      packId: "debuggatha/typescript",
      statement:
        "Prefer string enums or union types over numeric enums. If using numeric enums, always explicitly assign values.",
      category: "architecture",
      appliesTo: { kind: "requires-language", language: "typescript" },
      defaultSeverity: "medium",
      knowledgeRefs: ["know-typescript-enums"],
      contradicts: undefined,
    },
    {
      id: "prefer-as-const",
      packId: "debuggatha/typescript",
      statement:
        "Use `as const` to infer the narrowest possible literal types rather than explicitly typing out literal unions.",
      category: "style",
      appliesTo: { kind: "requires-language", language: "typescript" },
      defaultSeverity: "low",
      knowledgeRefs: ["know-typescript-as-const"],
      contradicts: undefined,
    },
  ],
  knowledge: [
    {
      id: "know-typescript-any",
      title: "The Danger of `any`",
      body: "The `any` type completely disables TypeScript's type checking for that value, propagating untyped data throughout the system. The `unknown` type is the type-safe counterpart; it requires explicit narrowing (via `typeof`, `instanceof`, or custom type guards) before use.",
      externalRefs: ["https://www.typescriptlang.org/docs/handbook/2/everyday-types.html#any"],
    },
    {
      id: "know-typescript-nullability",
      title: "Non-Null Assertions Subvert Type Safety",
      body: "Using `!` tells the compiler to ignore potential `null` or `undefined` values. This directly causes `TypeError: Cannot read properties of undefined` at runtime if the assumption is wrong. Control flow narrowing (`if (x != null)`) or optional chaining (`x?.y`) provides actual safety.",
      externalRefs: [
        "https://www.typescriptlang.org/docs/handbook/release-notes/typescript-2-0.html#non-null-assertion-operator",
      ],
    },
    {
      id: "know-typescript-promises",
      title: "Floating Promises",
      body: "An unhandled promise rejection in Node.js can crash the process. Fire-and-forget promises that are not awaited and lack a `.catch()` block will silently fail, obscuring critical errors in asynchronous operations.",
      externalRefs: ["https://typescript-eslint.io/rules/no-floating-promises/"],
    },
    {
      id: "know-typescript-type-imports",
      title: "Type-Only Imports",
      body: "Using `import type` guarantees that the import will be completely erased from the emitted JavaScript. This prevents circular dependency issues at runtime and allows build tools (like esbuild or swc) to safely drop the import without deep type analysis.",
      externalRefs: [
        "https://www.typescriptlang.org/docs/handbook/release-notes/typescript-3-8.html#type-only-imports-and-export",
      ],
    },
    {
      id: "know-typescript-return-types",
      title: "Explicit Return Types for Public APIs",
      body: "While type inference is powerful, relying on it for exported API surfaces can inadvertently expose internal types or cause downstream compilation errors if the implementation changes. Explicit return types enforce the API contract.",
      externalRefs: ["https://typescript-eslint.io/rules/explicit-module-boundary-types/"],
    },
    {
      id: "know-typescript-immutability",
      title: "Immutability via Readonly",
      body: "Marking object properties, arrays, and tuples as `readonly` provides compile-time guarantees against accidental mutation. This is particularly important for state management in reactive frameworks (like React or Redux).",
      externalRefs: [
        "https://www.typescriptlang.org/docs/handbook/typescript-in-5-minutes-func.html#readonly-and-const",
      ],
    },
    {
      id: "know-typescript-nullish",
      title: "Nullish Coalescing vs Logical OR",
      body: 'The `||` operator triggers on any falsy value, meaning `0`, `""`, and `false` will be incorrectly overridden by the default value. The `??` operator safely only triggers on strictly `null` or `undefined`.',
      externalRefs: [
        "https://www.typescriptlang.org/docs/handbook/release-notes/typescript-3-7.html#nullish-coalescing",
      ],
    },
    {
      id: "know-typescript-enums",
      title: "TypeScript Enum Pitfalls",
      body: "Numeric enums without explicit initializers are fundamentally unsafe in TypeScript. They allow reverse mapping and permit arbitrary numbers to be assigned to them, breaking type safety. String union types (`type Direction = 'UP' | 'DOWN'`) are significantly safer.",
      externalRefs: ["https://www.typescriptlang.org/docs/handbook/enums.html"],
    },
    {
      id: "know-typescript-as-const",
      title: "Const Assertions",
      body: 'The `as const` assertion tells the compiler to infer the most specific literal type possible, making properties `readonly` and preventing widening (e.g., inferring `"GET"` instead of `string`). This heavily reduces boilerplate.',
      externalRefs: [
        "https://www.typescriptlang.org/docs/handbook/release-notes/typescript-3-4.html#const-assertions",
      ],
    },
  ],
};
