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
      limitations: [
        "A route that validates input manually with a library like `zod` or `joi` inside the handler body (rather than via Fastify's schema option) is not necessarily insecure — flagging every route lacking a `schema` property as a finding would false-positive against equally-safe manual validation that just isn't wired through ajv.",
      ],
    },
    {
      id: "know-fastify-serialization",
      title: "Fast JSON Stringify",
      body: "If a response schema is provided, Fastify uses `fast-json-stringify` to serialize the output. This is 2x faster than `JSON.stringify()`. Furthermore, it guarantees that undeclared properties (like password hashes inadvertently attached to user objects) are stripped from the response.",
      externalRefs: [
        "https://fastify.dev/docs/latest/Reference/Validation-and-Serialization/#serialization",
      ],
      limitations: [
        "Endpoints that intentionally return a dynamic/heterogeneous shape (e.g. a generic proxy or admin debug endpoint never exposed to untrusted clients) may reasonably skip a response schema — treating every schema-less response as a data-leakage risk ignores that the leakage risk is specific to endpoints handling sensitive, user-facing objects.",
      ],
    },
    {
      id: "know-fastify-encapsulation",
      title: "Plugin Architecture",
      body: "Fastify uses an encapsulation model where child contexts inherit from parent contexts, but siblings cannot see each other's decorators. Wrapping a plugin in `fastify-plugin` breaks this encapsulation, polluting the global context. Use it deliberately.",
      externalRefs: ["https://fastify.dev/docs/latest/Reference/Encapsulation/"],
      limitations: [
        "A plugin that legitimately needs to register a shared decorator, hook, or database connection consumed by sibling plugins (the documented use case for `fastify-plugin`) is correctly using `fp`, not violating encapsulation — flagging every `fp()`-wrapped plugin as a finding ignores that this is the intended escape hatch, not a misuse by default.",
      ],
    },
    {
      id: "know-fastify-hooks",
      title: "Lifecycle Hooks",
      body: "Express middlewares execute sequentially and must manage `next()`. Fastify's lifecycle hooks (`onRequest`, `preHandler`, etc.) are deeply integrated into the framework's state machine, providing granular control and higher performance.",
      externalRefs: ["https://fastify.dev/docs/latest/Reference/Hooks/"],
      limitations: [
        "Migrating a large existing Express codebase incrementally onto Fastify is a legitimate reason to keep `@fastify/express` temporarily — flagging its presence as a defect ignores that it's an officially supported migration bridge, not exclusively a performance anti-pattern.",
      ],
    },
  ],
};
