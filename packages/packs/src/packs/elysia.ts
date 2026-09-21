import type { ReviewPack } from "@debuggatha/core";

export const elysiaPack: ReviewPack = {
  id: "debuggatha/elysia",
  version: "1.0.0",
  kind: "stack",
  displayName: "Elysia",
  dependsOn: [],
  rules: [
    {
      id: "use-typebox",
      packId: "debuggatha/elysia",
      statement:
        "Use Elysia's built-in `t` (TypeBox) validator for all route schemas to enable End-to-End type safety and automatic OpenAPI generation.",
      category: "architecture",
      appliesTo: { kind: "requires-framework", framework: "elysia" },
      defaultSeverity: "high",
      knowledgeRefs: ["know-elysia-typebox"],
      contradicts: undefined,
    },
    {
      id: "plugin-chaining",
      packId: "debuggatha/elysia",
      statement:
        "Chain Elysia instances using `.use()` to compose features. Do not mutate a single global Elysia instance.",
      category: "architecture",
      appliesTo: { kind: "requires-framework", framework: "elysia" },
      defaultSeverity: "medium",
      knowledgeRefs: ["know-elysia-plugins"],
      contradicts: undefined,
    },
    {
      id: "avoid-any",
      packId: "debuggatha/elysia",
      statement:
        "Do not cast Eden treaty client responses to `any`. Rely entirely on the inferred types from the Elysia server.",
      category: "style",
      appliesTo: { kind: "requires-framework", framework: "elysia" },
      defaultSeverity: "high",
      knowledgeRefs: ["know-elysia-eden"],
      contradicts: undefined,
    },
  ],
  knowledge: [
    {
      id: "know-elysia-typebox",
      title: "TypeBox Integration",
      body: "Elysia deeply integrates with TypeBox, which allows it to compile validators ahead of time using Bun's runtime, achieving validation speeds much faster than Zod while automatically generating Swagger docs.",
      externalRefs: ["https://elysiajs.com/concept/schema.html"],
      limitations: [
        "Internal, non-boundary routes used only for server-to-server or trusted health-check purposes with fixed, hardcoded request shapes gain little from `t` validation — requiring TypeBox schemas uniformly on every route regardless of whether it receives untrusted external input overstates the need for routes that are effectively internal-only.",
      ],
    },
    {
      id: "know-elysia-plugins",
      title: "Local State Encapsulation",
      body: "Elysia uses a builder pattern. Chaining `.use()` allows plugins to contribute types, state, and routes to the main instance without polluting global state.",
      externalRefs: ["https://elysiajs.com/concept/plugin.html"],
      limitations: [
        "Mutating the main instance directly with `.decorate()`/`.state()` at the top level of a small, single-file app (rather than composing via `.use()`) is a reasonable simplification for apps that will never grow plugin boundaries — flagging this in every codebase regardless of app size/complexity treats a scale-appropriate simplification as a defect.",
      ],
    },
    {
      id: "know-elysia-eden",
      title: "Eden End-to-End Type Safety",
      body: "The Eden client perfectly infers the exact structure, status codes, and errors from the Elysia backend. Bypassing these types with `any` defeats the core value proposition of the framework.",
      externalRefs: ["https://elysiajs.com/eden/overview.html"],
      limitations: [
        "Casting to `any` at a genuine external-integration boundary (e.g. wrapping a third-party webhook payload that isn't itself typed via Eden) is unrelated to this rule's concern — it targets casting away Eden's own inferred client types specifically, not all `any` usage in a codebase that happens to use Elysia.",
      ],
    },
  ],
};
