import type { ReviewPack } from "@debuggatha/knowledge-system";

export const expressPack: ReviewPack = {
  id: "debuggatha/express",
  version: "1.0.0",
  kind: "stack",
  displayName: "Express",
  dependsOn: [],
  rules: [
    {
      id: "async-route-errors",
      packId: "debuggatha/express",
      statement:
        "Always wrap async route handlers in a `try/catch` and pass errors to `next(err)`, or use an async error wrapper (like `express-async-errors`).",
      category: "architecture",
      appliesTo: { kind: "requires-framework", framework: "express" },
      defaultSeverity: "critical",
      knowledgeRefs: ["know-express-async-errors"],
      contradicts: undefined,
    },
    {
      id: "always-return-responses",
      packId: "debuggatha/express",
      statement:
        "Always prefix response calls with `return` (e.g., `return res.status(200).send()`) to prevent execution of subsequent code after sending the response.",
      category: "architecture",
      appliesTo: { kind: "requires-framework", framework: "express" },
      defaultSeverity: "high",
      knowledgeRefs: ["know-express-double-send"],
      contradicts: undefined,
    },
    {
      id: "disable-x-powered-by",
      packId: "debuggatha/express",
      statement: "Disable the `X-Powered-By` header using `app.disable('x-powered-by')` or helmet.",
      category: "security",
      appliesTo: { kind: "requires-framework", framework: "express" },
      defaultSeverity: "low",
      knowledgeRefs: ["know-express-x-powered-by"],
      contradicts: undefined,
    },
    {
      id: "use-helmet",
      packId: "debuggatha/express",
      statement:
        "Use the `helmet` middleware to set secure HTTP headers (CSP, HSTS, Frameguard, etc).",
      category: "security",
      appliesTo: { kind: "requires-framework", framework: "express" },
      defaultSeverity: "high",
      knowledgeRefs: ["know-express-helmet"],
      contradicts: undefined,
    },
    {
      id: "validate-input",
      packId: "debuggatha/express",
      statement:
        "Never trust `req.body` or `req.query`. Always validate and sanitize input against a strict schema (e.g., Zod or Joi) before processing.",
      category: "security",
      appliesTo: { kind: "requires-framework", framework: "express" },
      defaultSeverity: "critical",
      knowledgeRefs: ["know-express-validation"],
      contradicts: undefined,
    },
  ],
  knowledge: [
    {
      id: "know-express-async-errors",
      title: "Unhandled Promise Rejections in Express",
      body: "Express v4 does not catch unhandled promise rejections in async route handlers. If an async route throws, the request hangs until timeout, and the server process may crash depending on the Node version.",
      externalRefs: ["https://expressjs.com/en/guide/error-handling.html"],
      limitations: [
        "Express 5 (released as stable in 2024) natively catches rejected promises returned from async handlers and forwards them to the error-handling middleware automatically — this rule's premise (v4's lack of built-in handling) is a false positive when applied to an Express 5 codebase that doesn't need `express-async-errors` or manual try/catch at all.",
      ],
    },
    {
      id: "know-express-double-send",
      title: "ERR_HTTP_HEADERS_SENT",
      body: "Calling `res.send()` does not stop function execution. If code continues and calls `res.send()` again, the Node runtime will crash with `ERR_HTTP_HEADERS_SENT`. Using `return res.send()` prevents this.",
      externalRefs: [
        "https://nodejs.org/api/http.html#responsewriteheadstatuscode-statusmessage-headers",
      ],
      limitations: [
        "A response call that is genuinely the last statement in its function body (nothing executes after it, so a missing `return` has no double-send risk) is safe despite lacking the `return` keyword — a rule requiring `return` on every response call, rather than only ones followed by further executable code in the same path, will false-positive on this common case.",
      ],
    },
    {
      id: "know-express-x-powered-by",
      title: "Information Leakage",
      body: "The `X-Powered-By` header broadcasts the technology stack to potential attackers, aiding in targeted vulnerability scanning.",
      externalRefs: [
        "https://expressjs.com/en/advanced/best-practice-security.html#disable-x-powered-by-header",
      ],
      limitations: [
        "Removing the `X-Powered-By` header alone doesn't hide the stack from a determined attacker — response timing, error page shape, or other headers (e.g. a framework-specific cookie name) can still fingerprint Express, so this is defense-in-depth, not a rule whose absence implies the stack is actually undiscoverable.",
      ],
    },
    {
      id: "know-express-helmet",
      title: "Secure HTTP Headers",
      body: "Out of the box, Express misses critical security headers. Helmet configures Content Security Policy, prevents Clickjacking (X-Frame-Options), and enforces Strict-Transport-Security (HSTS).",
      externalRefs: ["https://expressjs.com/en/advanced/best-practice-security.html#use-helmet"],
      limitations: [
        "An app that sets the equivalent headers manually (its own CSP/HSTS/X-Frame-Options middleware) or sits behind a reverse proxy/CDN (e.g. Cloudflare, an API gateway) that already injects these headers doesn't need `helmet` itself — flagging the absence of the `helmet` package specifically, rather than the absence of the headers it would set, produces a false positive.",
      ],
    },
    {
      id: "know-express-validation",
      title: "Input Validation",
      body: "Assuming the structure of `req.body` leads to Prototype Pollution, NoSQL injection (in MongoDB), and type confusion crashes. Strict schema validation is a mandatory boundary.",
      externalRefs: ["https://zod.dev/", "https://joi.dev/"],
      limitations: [
        "Validation performed inside a shared middleware applied via `router.use()` upstream of the handler (rather than inline in the handler itself) satisfies this rule even though the handler's own code shows no visible validation call — a rule that only inspects the handler function body in isolation, without tracing the middleware chain, produces a false positive by missing that upstream validation.",
      ],
    },
  ],
};
