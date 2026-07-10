import type { ReviewPack } from "@debuggatha/knowledge-system";

export const goPack: ReviewPack = {
  id: "debuggatha/go",
  version: "1.0.0",
  kind: "stack",
  displayName: "Go",
  dependsOn: [],
  rules: [
    {
      id: "explicit-error-check",
      packId: "debuggatha/go",
      statement:
        "Always check returned errors using `if err != nil`. Never assign an error to the blank identifier `_`.",
      category: "architecture",
      appliesTo: { kind: "requires-language", language: "go" },
      defaultSeverity: "critical",
      knowledgeRefs: ["know-go-error-checking"],
      contradicts: undefined,
    },
    {
      id: "prevent-goroutine-leaks",
      packId: "debuggatha/go",
      statement:
        "Ensure every goroutine has a guaranteed exit condition via context cancellation or closed channels.",
      category: "performance",
      appliesTo: { kind: "requires-language", language: "go" },
      defaultSeverity: "high",
      knowledgeRefs: ["know-go-goroutine-leaks"],
      contradicts: undefined,
    },
    {
      id: "no-panic",
      packId: "debuggatha/go",
      statement:
        "Avoid `panic()`. Use it only for unrecoverable initialization errors (like missing config), never for control flow or request handling.",
      category: "architecture",
      appliesTo: { kind: "requires-language", language: "go" },
      defaultSeverity: "high",
      knowledgeRefs: ["know-go-panic"],
      contradicts: undefined,
    },
    {
      id: "defer-cleanup",
      packId: "debuggatha/go",
      statement:
        "Use `defer` immediately after opening a resource (files, locks, connections) to guarantee cleanup.",
      category: "architecture",
      appliesTo: { kind: "requires-language", language: "go" },
      defaultSeverity: "medium",
      knowledgeRefs: ["know-go-defer"],
      contradicts: undefined,
    },
    {
      id: "prefer-interfaces",
      packId: "debuggatha/go",
      statement:
        "Define interfaces where they are consumed, not where they are implemented. Keep interfaces small (1-2 methods).",
      category: "architecture",
      appliesTo: { kind: "requires-language", language: "go" },
      defaultSeverity: "medium",
      knowledgeRefs: ["know-go-interfaces"],
      contradicts: undefined,
    },
    {
      id: "value-vs-pointer",
      packId: "debuggatha/go",
      statement:
        "Pass small structs by value to avoid heap escapes and GC pressure. Pass by pointer only when mutating the receiver or copying large structs.",
      category: "performance",
      appliesTo: { kind: "requires-language", language: "go" },
      defaultSeverity: "medium",
      knowledgeRefs: ["know-go-pointers"],
      contradicts: undefined,
    },
    {
      id: "no-global-state",
      packId: "debuggatha/go",
      statement:
        "Avoid mutable global variables (e.g., `var DB *sql.DB`). Pass dependencies explicitly via struct fields.",
      category: "architecture",
      appliesTo: { kind: "requires-language", language: "go" },
      defaultSeverity: "medium",
      knowledgeRefs: ["know-go-global-state"],
      contradicts: undefined,
    },
    {
      id: "context-propagation",
      packId: "debuggatha/go",
      statement:
        "Always pass `context.Context` as the first argument to blocking operations or functions that span API boundaries.",
      category: "architecture",
      appliesTo: { kind: "requires-language", language: "go" },
      defaultSeverity: "high",
      knowledgeRefs: ["know-go-context"],
      contradicts: undefined,
    },
  ],
  knowledge: [
    {
      id: "know-go-error-checking",
      title: "Explicit Error Handling",
      body: "Go relies on values to communicate errors. Ignoring an error value by assigning it to `_` forces the program into an undefined state where subsequent logic runs against invalid data, often leading to panics.",
      externalRefs: ["https://go.dev/doc/effective_go#errors"],
    },
    {
      id: "know-go-goroutine-leaks",
      title: "Goroutine Leaks",
      body: "A goroutine blocked on a channel that will never be written to or read from stays in memory forever, leaking its stack and variables. Always bind goroutine lifecycles to a `context.Context`.",
      externalRefs: ["https://go101.org/article/memory-leaking.html"],
    },
    {
      id: "know-go-panic",
      title: "Panic vs Error",
      body: "Panics bypass the normal control flow and can crash the entire binary. They should strictly be reserved for 'impossible' invariants and fatal startup conditions, not for routing errors or malformed input.",
      externalRefs: ["https://go.dev/doc/effective_go#panic"],
    },
    {
      id: "know-go-defer",
      title: "Resource Cleanup via Defer",
      body: "Calling `defer file.Close()` immediately after successful opening ensures that regardless of how many early returns or errors occur later in the function, the resource is deterministically freed.",
      externalRefs: ["https://go.dev/tour/flowcontrol/12"],
    },
    {
      id: "know-go-interfaces",
      title: "Consumer-Defined Interfaces",
      body: "Unlike Java or C#, Go interfaces are satisfied implicitly. This means packages implementing concrete types do not need to declare interfaces. Consumers should declare small, targeted interfaces (like `io.Reader`) specifically for the behavior they require.",
      externalRefs: ["https://go.dev/doc/effective_go#interfaces_and_types"],
    },
    {
      id: "know-go-pointers",
      title: "Escape Analysis and GC Pressure",
      body: "Passing pointers forces the compiler to allocate the variable on the heap, increasing Garbage Collection pressure. For small structs, passing by value is significantly faster because it stays on the stack.",
      externalRefs: [
        "https://segment.com/blog/allocation-efficiency-in-high-performance-go-services/",
      ],
    },
    {
      id: "know-go-global-state",
      title: "Dependency Injection over Globals",
      body: "Global state makes concurrent tests impossible and hides dependencies. Struct-based dependency injection allows mocks to be easily swapped and isolates behavior safely.",
      externalRefs: [],
    },
    {
      id: "know-go-context",
      title: "Context Propagation",
      body: "The `context` package carries deadlines, cancellation signals, and request-scoped values across API boundaries. Failing to propagate context means downstream requests cannot be cancelled when a client disconnects.",
      externalRefs: ["https://pkg.go.dev/context"],
    },
  ],
};
