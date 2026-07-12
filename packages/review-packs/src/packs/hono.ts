import type { ReviewPack } from "@debuggatha/knowledge-system";

export const honoPack: ReviewPack = {
  id: "debuggatha/hono",
  version: "1.0.0",
  kind: "stack",
  displayName: "Hono",
  dependsOn: [],
  rules: [
    {
      id: "use-context-helpers",
      packId: "debuggatha/hono",
      statement:
        "Use Hono's Context `c` methods (`c.json()`, `c.text()`) instead of raw `Response` objects when returning standard data.",
      category: "architecture",
      appliesTo: { kind: "requires-framework", framework: "hono" },
      defaultSeverity: "low",
      knowledgeRefs: ["know-hono-context"],
      contradicts: undefined,
    },
    {
      id: "zod-validator",
      packId: "debuggatha/hono",
      statement:
        "Use `@hono/zod-validator` for request validation to ensure type safety between the client and the route handler (RPC-like types).",
      category: "architecture",
      appliesTo: { kind: "requires-framework", framework: "hono" },
      defaultSeverity: "high",
      knowledgeRefs: ["know-hono-validator"],
      contradicts: undefined,
    },
    {
      id: "edge-compatibility",
      packId: "debuggatha/hono",
      statement:
        "Avoid using Node.js core modules (like `fs` or `crypto`) directly in Hono routes unless using the explicitly polyfilled Node adapter. Hono is designed for Web Standard edge runtimes (Cloudflare Workers, Deno, Bun).",
      category: "architecture",
      appliesTo: { kind: "requires-framework", framework: "hono" },
      defaultSeverity: "critical",
      knowledgeRefs: ["know-hono-edge"],
      contradicts: undefined,
    },
  ],
  knowledge: [
    {
      id: "know-hono-context",
      title: "Context Object Ergonomics",
      body: "While Hono fully supports returning raw Web Standard `Response` objects, the `c` Context object provides highly optimized helpers that automatically set the correct headers and handle serialization.",
      externalRefs: ["https://hono.dev/api/context"],
      limitations: [
        "Returning a raw `Response` is required, not a style violation, when streaming a body, proxying an upstream `fetch()` response through unmodified, or setting headers/status combinations `c.json()`/`c.text()` don't support directly — flagging every raw `Response` return as a finding ignores these legitimate cases.",
      ],
    },
    {
      id: "know-hono-validator",
      title: "Type-Safe RPC",
      body: "When using Zod validation middleware in Hono, the inferred types of the request body and query parameters are automatically passed through to the handler, and can be shared with a frontend client using `hono/client`.",
      externalRefs: ["https://hono.dev/guides/validation"],
      limitations: [
        "An internal-only route with no external client (e.g. a health-check or metrics endpoint with no meaningful input) has no real need for `@hono/zod-validator`, and a pure backend-for-backend service with no `hono/client` consumer gets no RPC type-sharing benefit either — flagging its absence uniformly across all routes overstates the case for routes where neither justification applies.",
      ],
    },
    {
      id: "know-hono-edge",
      title: "Edge First Design",
      body: "Hono relies on Web Standards (`Request`, `Response`, `fetch`). Importing Node-specific modules destroys the portability of the application, preventing deployment to Cloudflare Workers or Deno.",
      externalRefs: ["https://hono.dev/getting-started/basic"],
      limitations: [
        "A Hono app explicitly targeting the Node.js adapter (`@hono/node-server`) as its only intended deployment target has no portability requirement to preserve, so importing `fs` or `crypto` there is a legitimate architectural choice, not a violation — this rule applies specifically to apps intended for edge/multi-runtime deployment.",
      ],
    },
  ],
};
