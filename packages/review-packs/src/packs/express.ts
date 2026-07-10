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
    },
    {
      id: "know-express-double-send",
      title: "ERR_HTTP_HEADERS_SENT",
      body: "Calling `res.send()` does not stop function execution. If code continues and calls `res.send()` again, the Node runtime will crash with `ERR_HTTP_HEADERS_SENT`. Using `return res.send()` prevents this.",
      externalRefs: [],
    },
    {
      id: "know-express-x-powered-by",
      title: "Information Leakage",
      body: "The `X-Powered-By` header broadcasts the technology stack to potential attackers, aiding in targeted vulnerability scanning.",
      externalRefs: [
        "https://expressjs.com/en/advanced/best-practice-security.html#disable-x-powered-by-header",
      ],
    },
    {
      id: "know-express-helmet",
      title: "Secure HTTP Headers",
      body: "Out of the box, Express misses critical security headers. Helmet configures Content Security Policy, prevents Clickjacking (X-Frame-Options), and enforces Strict-Transport-Security (HSTS).",
      externalRefs: ["https://expressjs.com/en/advanced/best-practice-security.html#use-helmet"],
    },
    {
      id: "know-express-validation",
      title: "Input Validation",
      body: "Assuming the structure of `req.body` leads to Prototype Pollution, NoSQL injection (in MongoDB), and type confusion crashes. Strict schema validation is a mandatory boundary.",
      externalRefs: [],
    },
  ],
};
