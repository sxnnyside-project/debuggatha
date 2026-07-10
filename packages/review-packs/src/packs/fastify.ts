import type { ReviewPack } from "@debuggatha/knowledge-system";

export const fastifyPack: ReviewPack = {
  id: "debuggatha/fastify",
  version: "1.0.0",
  kind: "stack",
  displayName: "Fastify",
  dependsOn: [],
  rules: [
    {
      id: "schema-validation",
      packId: "debuggatha/fastify",
      statement:
        "Always define JSON schemas for route validation (querystring, params, headers, body) to leverage fastify's built-in `ajv` compilation.",
      category: "security",
      appliesTo: { kind: "requires-framework", framework: "fastify" },
      defaultSeverity: "high",
      knowledgeRefs: ["know-fastify-validation"],
      contradicts: undefined,
    },
    {
      id: "schema-serialization",
      packId: "debuggatha/fastify",
      statement:
        "Define response schemas (2xx codes) to enable `fast-json-stringify`. This drastically improves response serialization performance and prevents accidental data leakage.",
      category: "performance",
      appliesTo: { kind: "requires-framework", framework: "fastify" },
      defaultSeverity: "high",
      knowledgeRefs: ["know-fastify-serialization"],
      contradicts: undefined,
    },
    {
      id: "plugin-encapsulation",
      packId: "debuggatha/fastify",
      statement:
        "Use `fastify-plugin` (fp) only when a plugin must expose decorators or hooks to the parent scope. Otherwise, allow normal encapsulation.",
      category: "architecture",
      appliesTo: { kind: "requires-framework", framework: "fastify" },
      defaultSeverity: "medium",
      knowledgeRefs: ["know-fastify-encapsulation"],
      contradicts: undefined,
    },
    {
      id: "avoid-express-middlewares",
      packId: "debuggatha/fastify",
      statement:
        "Avoid using `@fastify/express` to run Express middlewares unless absolutely necessary. Rely on Fastify's native lifecycle hooks instead.",
      category: "performance",
      appliesTo: { kind: "requires-framework", framework: "fastify" },
      defaultSeverity: "medium",
      knowledgeRefs: ["know-fastify-hooks"],
      contradicts: undefined,
    },
  ],
  knowledge: [
    {
      id: "know-fastify-validation",
      title: "AJV Schema Validation",
      body: "Fastify compiles JSON schemas into highly optimized validation functions at startup. This provides robust security against malformed input and is significantly faster than doing manual validation inside the route handler.",
      externalRefs: ["https://fastify.dev/docs/latest/Reference/Validation-and-Serialization/"],
    },
    {
      id: "know-fastify-serialization",
      title: "Fast JSON Stringify",
      body: "If a response schema is provided, Fastify uses `fast-json-stringify` to serialize the output. This is 2x faster than `JSON.stringify()`. Furthermore, it guarantees that undeclared properties (like password hashes inadvertently attached to user objects) are stripped from the response.",
      externalRefs: [
        "https://fastify.dev/docs/latest/Reference/Validation-and-Serialization/#serialization",
      ],
    },
    {
      id: "know-fastify-encapsulation",
      title: "Plugin Architecture",
      body: "Fastify uses an encapsulation model where child contexts inherit from parent contexts, but siblings cannot see each other's decorators. Wrapping a plugin in `fastify-plugin` breaks this encapsulation, polluting the global context. Use it deliberately.",
      externalRefs: ["https://fastify.dev/docs/latest/Reference/Encapsulation/"],
    },
    {
      id: "know-fastify-hooks",
      title: "Lifecycle Hooks",
      body: "Express middlewares execute sequentially and must manage `next()`. Fastify's lifecycle hooks (`onRequest`, `preHandler`, etc.) are deeply integrated into the framework's state machine, providing granular control and higher performance.",
      externalRefs: ["https://fastify.dev/docs/latest/Reference/Hooks/"],
    },
  ],
};
