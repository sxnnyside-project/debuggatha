import type { ReviewPack } from "@debuggatha/knowledge-system";

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
      category: "dx",
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
      category: "dx",
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
    },
    {
      id: "know-bun-file",
      title: "Zero-Copy File I/O",
      body: "`Bun.file()` returns a `BunFile` object that can be passed directly as an HTTP Response body. Bun will automatically use `sendfile` to stream the file directly from the kernel to the network socket, achieving zero-copy I/O.",
      externalRefs: ["https://bun.sh/docs/api/file-io"],
    },
    {
      id: "know-bun-test",
      title: "Native Testing Ecosystem",
      body: "`bun:test` is built directly into the runtime. It is Jest-compatible but avoids the overhead of Babel/tsc transpilation and heavy test runner boot times.",
      externalRefs: ["https://bun.sh/docs/cli/test"],
    },
    {
      id: "know-bun-sqlite",
      title: "Native SQLite Driver",
      body: "The `bun:sqlite` module is a high-performance native driver built directly into Bun. It significantly outperforms `better-sqlite3` by avoiding Node.js N-API/FFI overhead.",
      externalRefs: ["https://bun.sh/docs/api/sqlite"],
    },
    {
      id: "know-bun-cli",
      title: "Fast Script Execution",
      body: "`bun run` uses Bun's fast startup time to execute scripts up to 100x faster than `npm run`, improving local developer velocity.",
      externalRefs: ["https://bun.sh/docs/cli/run"],
    },
  ],
};
