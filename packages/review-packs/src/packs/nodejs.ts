import type { ReviewPack } from "@debuggatha/knowledge-system";

export const nodejsPack: ReviewPack = {
  id: "debuggatha/nodejs",
  version: "1.0.0",
  kind: "stack",
  displayName: "Node.js",
  dependsOn: [],
  rules: [
    {
      id: "no-sync-io",
      packId: "debuggatha/nodejs",
      statement:
        "Never use synchronous I/O methods (e.g., `fs.readFileSync`) during request handling or in the main event loop.",
      category: "performance",
      appliesTo: { kind: "requires-language", language: "nodejs" },
      defaultSeverity: "critical",
      knowledgeRefs: ["know-nodejs-event-loop"],
      contradicts: undefined,
    },
    {
      id: "handle-unhandled-rejections",
      packId: "debuggatha/nodejs",
      statement:
        "Do not leave unhandled promise rejections. In newer Node versions, this will terminate the process immediately.",
      category: "architecture",
      appliesTo: { kind: "requires-language", language: "nodejs" },
      defaultSeverity: "high",
      knowledgeRefs: ["know-nodejs-rejections"],
      contradicts: undefined,
    },
    {
      id: "prefer-node-protocol",
      packId: "debuggatha/nodejs",
      statement:
        "Always use the `node:` protocol when importing core modules (e.g., `import path from 'node:path'`).",
      category: "security",
      appliesTo: { kind: "requires-language", language: "nodejs" },
      defaultSeverity: "low",
      knowledgeRefs: ["know-nodejs-protocol"],
      contradicts: undefined,
    },
    {
      id: "use-worker-threads",
      packId: "debuggatha/nodejs",
      statement:
        "Offload CPU-intensive tasks (like crypto hashing or image processing) to `worker_threads` to avoid blocking the event loop.",
      category: "performance",
      appliesTo: { kind: "requires-language", language: "nodejs" },
      defaultSeverity: "high",
      knowledgeRefs: ["know-nodejs-workers"],
      contradicts: undefined,
    },
    {
      id: "avoid-process-exit",
      packId: "debuggatha/nodejs",
      statement:
        "Avoid calling `process.exit()`. Throw an error and let the application gracefully shut down by closing servers and database connections first.",
      category: "architecture",
      appliesTo: { kind: "requires-language", language: "nodejs" },
      defaultSeverity: "medium",
      knowledgeRefs: ["know-nodejs-exit"],
      contradicts: undefined,
    },
    {
      id: "prefer-abort-controller",
      packId: "debuggatha/nodejs",
      statement:
        "Use `AbortController` to handle request cancellations and timeouts instead of relying on custom timeout logic or event emitters.",
      category: "architecture",
      appliesTo: { kind: "requires-language", language: "nodejs" },
      defaultSeverity: "medium",
      knowledgeRefs: ["know-nodejs-abort"],
      contradicts: undefined,
    },
  ],
  knowledge: [
    {
      id: "know-nodejs-event-loop",
      title: "Event Loop Blocking",
      body: "Node.js is single-threaded. Calling a synchronous method like `fs.readFileSync` completely blocks the thread, meaning no other HTTP requests can be processed until the file I/O completes. This is a severe DOS vulnerability in production.",
      externalRefs: ["https://nodejs.org/en/docs/guides/dont-block-the-event-loop/"],
      limitations: [
        "A one-time synchronous read at process startup, before the HTTP server begins accepting connections (e.g. `fs.readFileSync` to load a config file in the module's top-level scope), never blocks an in-flight request and is a standard, accepted pattern — flagging every `readFileSync` call uniformly, regardless of whether it runs during startup or during request handling, produces false positives.",
      ],
    },
    {
      id: "know-nodejs-rejections",
      title: "Unhandled Rejections",
      body: "Historically, unhandled promise rejections emitted a warning. Since Node.js 15+, they throw an exception and terminate the process. All promises must be awaited or caught.",
      externalRefs: ["https://nodejs.org/api/process.html#event-unhandledrejection"],
      limitations: [
        "A fire-and-forget promise intentionally left unawaited but with an explicit `.catch()` attached (or wrapped by a library's own rejection tracking, e.g. a queue system that logs and swallows job failures) is not an unhandled rejection even though it lacks an `await` — a scan that flags every non-awaited promise expression without checking for an attached `.catch()`/`.then(_, onRejected)` will false-positive on already-safe fire-and-forget code.",
      ],
    },
    {
      id: "know-nodejs-protocol",
      title: "Node Protocol Imports",
      body: "Using the `node:` prefix explicitly signals to the module resolver that the module is built-in. This prevents a supply chain attack where a malicious npm package mimics a core module name.",
      externalRefs: ["https://nodejs.org/api/modules.html#core-modules"],
      limitations: [
        "Code that must run on an older Node.js runtime predating widespread `node:` prefix support for a given module (some builtins only gained the prefixed form in later 12.x/14.x patch releases), or code sharing a codebase with browser-targeted bundles where `node:`-prefixed imports aren't resolvable by all bundlers, may have a legitimate compatibility reason for the bare specifier — flagging every unprefixed core-module import without accounting for the target runtime/bundler produces false positives in those environments.",
      ],
    },
    {
      id: "know-nodejs-workers",
      title: "CPU Bound Tasks",
      body: "Because the main thread handles all asynchronous I/O callbacks, running a synchronous CPU-bound task (like a heavy `while` loop) freezes the server. `worker_threads` allow true parallel execution in V8 isolates.",
      externalRefs: ["https://nodejs.org/api/worker_threads.html"],
      limitations: [
        "A CPU-bound task that's fast enough to complete in well under a millisecond (parsing a small JSON payload, hashing a short string) doesn't meaningfully block the event loop despite technically being synchronous compute — flagging every synchronous CPU operation as needing `worker_threads` ignores that worker communication overhead (structured-clone serialization across the thread boundary) can make small tasks slower, not faster, when offloaded.",
      ],
    },
    {
      id: "know-nodejs-exit",
      title: "Graceful Shutdowns",
      body: "`process.exit()` immediately kills the runtime, terminating all in-flight requests and leaving database transactions hanging. Allow the event loop to naturally drain, or manually close active servers.",
      externalRefs: ["https://nodejs.org/api/process.html#process_process_exit_code"],
      limitations: [
        "A CLI tool or one-shot script (not a long-running server) that has finished its work and wants to force-exit rather than wait on a stray open handle (e.g. an unclosed database pool keeping the event loop alive) is a legitimate, common use of `process.exit()` — flagging it identically to calling it inside a request handler of a running HTTP server ignores this different execution context.",
      ],
    },
    {
      id: "know-nodejs-abort",
      title: "Standardized Cancellation",
      body: "The Web Standard `AbortController` API is now universally supported across Node.js core modules (`fetch`, `fs`, `timers`, `stream`). It is the canonical way to propagate cancellation.",
      externalRefs: ["https://nodejs.org/api/globals.html#class-abortcontroller"],
      limitations: [
        "A codebase built on an older library version that predates that library's `AbortSignal` support (some ORMs, HTTP clients, and queue libraries added it late, or not at all for certain operations) cannot adopt `AbortController` for that specific call without an upstream version bump — flagging its absence without checking whether the underlying API actually accepts a signal produces a false positive on a real integration limitation, not a code smell.",
      ],
    },
  ],
};
