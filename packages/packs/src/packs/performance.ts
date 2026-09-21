import type { ReviewPack } from "@debuggatha/core";

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
      limitations: [
        "Regex engines with linear-time guarantees (e.g. RE2, or JavaScript's experimental `v8-re2`-backed paths) are not vulnerable to catastrophic backtracking even with nested quantifiers, so flagging the pattern shape alone without knowing the executing engine can false-positive against a safe regex engine.",
      ],
    },
    {
      id: "know-perf-n-plus-one",
      title: "N+1 Problem",
      body: "Fetching a list of N items and then looping through that list to fetch relations executes N+1 database queries. This is the most common cause of ORM performance degradation. It saturates database connections and spikes latency.",
      externalRefs: ["https://secure.phabricator.com/book/phabcontrib/article/n_plus_one/"],
      limitations: [
        "A query inside a loop over an in-memory, already-fetched array (not a fresh per-iteration round trip to a growing collection) or a loop bounded to a small fixed size (e.g. iterating a config array of 3 known keys) is not the N+1 pattern this rule targets, and a naive AST scan for 'query call inside loop' can false-positive on it.",
      ],
    },
    {
      id: "know-perf-pagination",
      title: "Unbounded Result Sets",
      body: "Returning all records from a database table works in development but crashes production environments when the table grows. It consumes immense amounts of RAM to serialize the JSON and causes massive network latency.",
      externalRefs: [],
      limitations: [
        "An endpoint backed by a table with a hard, enforced upper bound on row count (e.g. a singleton config table, or a per-tenant table capped by a foreign-key constraint to a small parent set) does not need pagination, so flagging every unbounded-looking query as a violation without knowing the table's growth characteristics can false-positive.",
      ],
    },
    {
      id: "know-perf-caching",
      title: "HTTP Caching",
      body: "Setting appropriate `Cache-Control` (like `public, max-age=31536000, immutable` for hashed assets) pushes the load entirely off the origin server onto CDNs and browser caches, massively reducing infrastructure costs.",
      externalRefs: ["https://developer.mozilla.org/en-US/docs/Web/HTTP/Caching"],
      limitations: [
        "Responses containing per-user or otherwise sensitive data must not be marked `public` or cached at a shared CDN even if they are idempotent — a rule that only checks 'is this GET/idempotent' without checking response sensitivity can push a false-positive recommendation to cache data that should stay `private` or `no-store`.",
      ],
    },
  ],
};
