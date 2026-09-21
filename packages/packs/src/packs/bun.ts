import type { ReviewPack } from "@debuggatha/core";

export const bunPack: ReviewPack = {
  id: "debuggatha/bun",
  version: "1.0.0",
  kind: "stack",
  displayName: "Bun",
  dependsOn: [],
  rules: [
    {
      id: "prefer-bun-serve",
      packId: "debuggatha/bun",
      statement:
        "Use `Bun.serve()` for HTTP servers instead of the Node.js `http` module. It is highly optimized and natively supports Web standard Request/Response.",
      category: "performance",
      appliesTo: { kind: "requires-language", language: "bun" },
      defaultSeverity: "high",
      knowledgeRefs: ["know-bun-serve"],
      contradicts: undefined,
    },
    {
      id: "use-bun-file",
      packId: "debuggatha/bun",
      statement:
        "Use `Bun.file()` for I/O operations instead of `fs.readFile`. It leverages the fastest system-level I/O mechanisms (like `sendfile`).",
      category: "performance",
      appliesTo: { kind: "requires-language", language: "bun" },
      defaultSeverity: "medium",
      knowledgeRefs: ["know-bun-file"],
      contradicts: undefined,
    },
    {
      id: "prefer-bun-test",
      packId: "debuggatha/bun",
      statement:
        "Use the built-in `bun:test` module instead of `jest` or `vitest`. It is significantly faster and natively supports TypeScript/JSX.",
      category: "style",
      appliesTo: { kind: "requires-language", language: "bun" },
      defaultSeverity: "low",
      knowledgeRefs: ["know-bun-test"],
      contradicts: undefined,
    },
    {
      id: "use-bun-sqlite",
      packId: "debuggatha/bun",
      statement:
        "Use `bun:sqlite` for local SQLite databases. It is a highly optimized, native driver that avoids FFI overhead.",
      category: "performance",
      appliesTo: { kind: "requires-language", language: "bun" },
      defaultSeverity: "medium",
      knowledgeRefs: ["know-bun-sqlite"],
      contradicts: undefined,
    },
    {
      id: "avoid-npm-run",
      packId: "debuggatha/bun",
      statement:
        "Run scripts directly using `bun run` instead of `npm run` to bypass the slow Node.js boot time.",
      category: "style",
      appliesTo: { kind: "requires-language", language: "bun" },
      defaultSeverity: "low",
      knowledgeRefs: ["know-bun-cli"],
      contradicts: undefined,
    },
  ],
  knowledge: [
    {
      id: "know-bun-serve",
      title: "Native Bun.serve Performance",
      body: "`Bun.serve()` is implemented natively in Zig and leverages Web standards (`Request`, `Response`). It avoids the heavy memory allocation and event emitter overhead of Node's `http` module.",
      externalRefs: ["https://bun.sh/docs/api/http"],
      limitations: [
        "Not a drop-in swap for code coupled to Node's `http.IncomingMessage`/`ServerResponse` shape — middleware written against that exact API (rather than the Web `Request`/`Response` standard) needs adaptation, not just a runtime switch.",
      ],
    },
    {
      id: "know-bun-file",
      title: "Zero-Copy File I/O",
      body: "`Bun.file()` returns a `BunFile` object that can be passed directly as an HTTP Response body. Bun will automatically use `sendfile` to stream the file directly from the kernel to the network socket, achieving zero-copy I/O.",
      externalRefs: ["https://bun.sh/docs/api/file-io"],
      limitations: [
        "The zero-copy path applies to serving/reading whole files as a Response body; code that needs partial/streamed reads, byte-range requests, or Node's callback-based `fs.readFile` for compatibility with a library expecting Node `Buffer`s doesn't get this benefit automatically and may need adaptation, not just substitution.",
      ],
    },
    {
      id: "know-bun-test",
      title: "Native Testing Ecosystem",
      body: "`bun:test` is built directly into the runtime. It is Jest-compatible but avoids the overhead of Babel/tsc transpilation and heavy test runner boot times.",
      externalRefs: ["https://bun.sh/docs/cli/test"],
      limitations: [
        '"Jest-compatible" is not 100% API parity — some `jest.mock` behaviors, custom snapshot serializers, or Babel-plugin-dependent transforms may not run unmodified under `bun:test`; suites that fail after migration are a compatibility gap, not necessarily a code defect.',
      ],
    },
    {
      id: "know-bun-sqlite",
      title: "Native SQLite Driver",
      body: "The `bun:sqlite` module is a high-performance native driver built directly into Bun. It significantly outperforms `better-sqlite3` by avoiding Node.js N-API/FFI overhead.",
      externalRefs: ["https://bun.sh/docs/api/sqlite"],
      limitations: [
        "`bun:sqlite` only runs under the Bun runtime — a package that must also run under plain Node.js (e.g. a shared library published to npm) cannot use it without a runtime-conditional fallback to `better-sqlite3` or similar.",
      ],
    },
    {
      id: "know-bun-cli",
      title: "Fast Script Execution",
      body: "`bun run` uses Bun's fast startup time to execute scripts up to 100x faster than `npm run`, improving local developer velocity.",
      externalRefs: ["https://bun.sh/docs/cli/run"],
      limitations: [
        "The speed gain is about script boot time, not semantics — scripts relying on npm-specific subcommands (e.g. `npm run --workspace`) or on npm's exact package-resolution/hoisting behavior can behave differently under Bun, so this is a performance recommendation, not a guaranteed behavior-preserving substitution.",
      ],
    },
  ],
};
