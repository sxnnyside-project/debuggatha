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
    },
    {
      id: "know-nodejs-rejections",
      title: "Unhandled Rejections",
      body: "Historically, unhandled promise rejections emitted a warning. Since Node.js 15+, they throw an exception and terminate the process. All promises must be awaited or caught.",
      externalRefs: ["https://nodejs.org/api/process.html#event-unhandledrejection"],
    },
    {
      id: "know-nodejs-protocol",
      title: "Node Protocol Imports",
      body: "Using the `node:` prefix explicitly signals to the module resolver that the module is built-in. This prevents a supply chain attack where a malicious npm package mimics a core module name.",
      externalRefs: ["https://nodejs.org/api/modules.html#core-modules"],
    },
    {
      id: "know-nodejs-workers",
      title: "CPU Bound Tasks",
      body: "Because the main thread handles all asynchronous I/O callbacks, running a synchronous CPU-bound task (like a heavy `while` loop) freezes the server. `worker_threads` allow true parallel execution in V8 isolates.",
      externalRefs: ["https://nodejs.org/api/worker_threads.html"],
    },
    {
      id: "know-nodejs-exit",
      title: "Graceful Shutdowns",
      body: "`process.exit()` immediately kills the runtime, terminating all in-flight requests and leaving database transactions hanging. Allow the event loop to naturally drain, or manually close active servers.",
      externalRefs: ["https://nodejs.org/api/process.html#process_process_exit_code"],
    },
    {
      id: "know-nodejs-abort",
      title: "Standardized Cancellation",
      body: "The Web Standard `AbortController` API is now universally supported across Node.js core modules (`fetch`, `fs`, `timers`, `stream`). It is the canonical way to propagate cancellation.",
      externalRefs: ["https://nodejs.org/api/globals.html#class-abortcontroller"],
    },
  ],
};
