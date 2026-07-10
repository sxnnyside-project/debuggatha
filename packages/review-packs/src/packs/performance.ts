import type { ReviewPack } from "@debuggatha/knowledge-system";

export const performancePack: ReviewPack = {
  id: "debuggatha/performance",
  version: "1.0.0",
  kind: "concern",
  displayName: "Performance",
  dependsOn: [],
  rules: [
    {
      id: "avoid-regex-dos",
      packId: "debuggatha/performance",
      statement:
        "Avoid deeply nested quantifiers (e.g., `(a+)+`) in Regular Expressions that are evaluated against untrusted input.",
      category: "security",
      appliesTo: { kind: "always" },
      defaultSeverity: "critical",
      knowledgeRefs: ["know-perf-redos"],
      contradicts: undefined,
    },
    {
      id: "n-plus-one-queries",
      packId: "debuggatha/performance",
      statement:
        "Do not execute database queries inside loops. Use batching (e.g., DataLoader) or explicit JOIN/eager-loading mechanisms.",
      category: "performance",
      appliesTo: { kind: "always" },
      defaultSeverity: "high",
      knowledgeRefs: ["know-perf-n-plus-one"],
      contradicts: undefined,
    },
    {
      id: "pagination",
      packId: "debuggatha/performance",
      statement:
        "Always implement pagination (cursor or offset) when returning collections of data from an API. Never return unbounded lists.",
      category: "architecture",
      appliesTo: { kind: "always" },
      defaultSeverity: "critical",
      knowledgeRefs: ["know-perf-pagination"],
      contradicts: undefined,
    },
    {
      id: "cache-headers",
      packId: "debuggatha/performance",
      statement: "Apply `Cache-Control` headers to static assets and idempotent API responses.",
      category: "performance",
      appliesTo: { kind: "always" },
      defaultSeverity: "medium",
      knowledgeRefs: ["know-perf-caching"],
      contradicts: undefined,
    },
  ],
  knowledge: [
    {
      id: "know-perf-redos",
      title: "Regular Expression Denial of Service",
      body: "Catastrophic backtracking occurs when a regex engine attempts to match an input string against a poorly constructed regex with nested quantifiers. An attacker can craft a 50-character string that takes the CPU hours to evaluate, immediately freezing the server.",
      externalRefs: [
        "https://owasp.org/www-community/attacks/Regular_expression_Denial_of_Service_-_ReDoS",
      ],
    },
    {
      id: "know-perf-n-plus-one",
      title: "N+1 Problem",
      body: "Fetching a list of N items and then looping through that list to fetch relations executes N+1 database queries. This is the most common cause of ORM performance degradation. It saturates database connections and spikes latency.",
      externalRefs: ["https://secure.phabricator.com/book/phabcontrib/article/n_plus_one/"],
    },
    {
      id: "know-perf-pagination",
      title: "Unbounded Result Sets",
      body: "Returning all records from a database table works in development but crashes production environments when the table grows. It consumes immense amounts of RAM to serialize the JSON and causes massive network latency.",
      externalRefs: [],
    },
    {
      id: "know-perf-caching",
      title: "HTTP Caching",
      body: "Setting appropriate `Cache-Control` (like `public, max-age=31536000, immutable` for hashed assets) pushes the load entirely off the origin server onto CDNs and browser caches, massively reducing infrastructure costs.",
      externalRefs: ["https://developer.mozilla.org/en-US/docs/Web/HTTP/Caching"],
    },
  ],
};
